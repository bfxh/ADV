"""检索评测集的**机械生成**（M4-1）：语料清单 + 查询 + 金标，全部确定性可重放。

用户拍板：语料用**混合语料**（本仓代码 + vendored noseyparker + RESEARCH 注册表文本），
查询用**机械生成（带扰动）**。机械生成天生有泄漏风险，所以本模块只产**候选**——
过不过门由 `protocol.verdict` 的三条判据说了算，过不了的作废并计数（**产率本身就是第一个读数**）。

确定性三条：
  ① 文件枚举全排序；② 每条查询的随机数只由「文档路径 + 查询种类」播种（不碰全局 random）；
  ③ 输出 JSONL 字段序固定、换行 LF。`--check` 就是拿这三点跟仓里的冻结文件逐字节对账。

用法：
  python -X utf8 bench/retrieval/gen_queries.py --write      # 写 corpus.sha256 + queries.jsonl
  python -X utf8 bench/retrieval/gen_queries.py --check      # 重放并与冻结文件逐字节对账
  python -X utf8 bench/retrieval/gen_queries.py --stats      # 只打印产率与语料规模
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import random
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import protocol  # noqa: E402（同目录脚本，按本仓 bench 脚本惯例用 sys.path 直插）

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
CORPUS_MANIFEST = HERE / "corpus.sha256"
QUERIES = HERE / "queries.jsonl"

#: 三源语料（glob 相对 ROOT；目录制 = 文档，jsonl 逐行 = 文档）。
SOURCES = [
    ("repo", ["crates/**/*.rs", "scripts/*.py"]),
    ("noseyparker", ["third_party/noseyparker/crates/**/*.rs"]),
    ("research", ["RESEARCH/registry/*.jsonl"]),
]
#: 遍历时要跳掉的目录名（与仓里其它扫描脚本同一口径）。
SKIP_DIRS = {".git", "target", "node_modules", "__pycache__", "data"}

_SYNONYMS = {
    "check": "verify", "build": "construct", "read": "load", "write": "store",
    "scan": "sweep", "find": "locate", "list": "enumerate", "run": "execute",
    "config": "settings", "gate": "guard", "report": "summary", "parse": "decode",
}


def _keep(path: pathlib.Path) -> bool:
    return not any(part in SKIP_DIRS for part in path.parts)


def pin_sha() -> str:
    """读 `corpus.pin` 里那行 `pin <sha>`（语料锚）。"""
    for line in (HERE / "corpus.pin").read_text(encoding="utf-8").splitlines():
        if line.startswith("pin "):
            return line.split()[1]
    raise SystemExit("corpus.pin 里没有 `pin <sha>` 行")


def _git_bytes(sha: str, paths: list[str]) -> dict[str, str]:
    """批量取 `sha` 下这些路径的内容（一次 `git cat-file --batch`，不是 2404 次子进程）。

    注意 `--batch` 回包里 header 里的数字是**字节数**不是行数——按行切会在含换行的文件上错位
    （第一版就是这么写的，实测 docs=0）。所以走二进制、按 size 精确切，再按 utf-8 解码。
    """
    payload = "\n".join(f"{sha}:{p}" for p in paths) + "\n"
    cp = subprocess.run(["git", "cat-file", "--batch"], input=payload.encode("utf-8"),
                        capture_output=True, cwd=str(ROOT))
    if cp.returncode != 0:
        raise SystemExit(f"git cat-file 失败：{cp.stderr.decode('utf-8', 'replace').strip()}")
    out: dict[str, str] = {}
    data = cp.stdout
    pos = 0
    for path in paths:
        eol = data.find(b"\n", pos)
        if eol < 0:
            break
        header = data[pos:eol].decode("utf-8", "replace")
        pos = eol + 1
        if header.endswith(" missing"):
            continue
        parts = header.split()
        size = int(parts[2])
        blob = data[pos:pos + size]
        pos += size + 1                      # 跳过内容后的那个换行
        out[path] = blob.decode("utf-8", "replace")
    return out


def _glob_to_regex(pattern: str):
    """仓内 glob → 正则：`**/` 任意层级、`*` 单层、其余字面。

    为什么要自己转：`fnmatch` 不认 `**`，用"子串近似"替代会把 `rust/`、`bench/` 这类
    不属于该源的路径也吸进来（实测：第一版就多吸了 700 个候选）。
    """
    out = ["^"]
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    out.append("$")
    return re.compile("".join(out))


def enumerate_docs(pin: str | None = None) -> list[tuple[str, str]]:
    """→ [(doc_id, text)]。doc_id 是仓相对路径（jsonl 加 `#行号`）；内容取自**钉版提交**。"""
    sha = pin or pin_sha()
    tree = subprocess.run(["git", "ls-tree", "-r", "--name-only", sha], capture_output=True,
                          text=True, encoding="utf-8", cwd=str(ROOT))
    if tree.returncode != 0:
        raise SystemExit(f"git ls-tree 失败（浅克隆里没有 {sha}？）：{tree.stderr.strip()}")
    all_paths = [p for p in tree.stdout.splitlines() if p and not any(s in p.split("/") for s in SKIP_DIRS)]
    wanted: list[str] = []
    for _source, globs in SOURCES:
        for g in globs:
            rx = _glob_to_regex(g)
            wanted.extend(p for p in all_paths if rx.match(p))
    wanted = sorted(set(wanted))
    blobs = _git_bytes(sha, wanted)
    docs: list[tuple[str, str]] = []
    for rel in wanted:
        text = blobs.get(rel, "")
        if rel.endswith(".jsonl"):
            for ln, line in enumerate(text.splitlines(), start=1):
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                joined = " ".join(str(obj.get(k, "")) for k in ("name", "mechanism", "defect", "why"))
                if joined.strip():
                    docs.append((f"{rel}#{ln}", joined))
        elif text:
            docs.append((rel, text))
    return docs


def symbols(text: str, lang_hint: str) -> list[str]:
    """文档里的函数/类型名（rust: `fn name`/`struct name`；python: `def name`/`class name`）。"""
    out = re.findall(r"\b(?:fn|def|class|struct|enum|trait|impl)\s+([A-Za-z_][A-Za-z0-9_]*)", text)
    if lang_hint == "code":
        out += re.findall(r"\b(?:const|static|type)\s+([A-Za-z_][A-Za-z0-9_]*)", text)
    return out


def first_sentence(text: str) -> str:
    """文档首句：优先取注释行（`///` / `#` / `//!`），退化到首个非空行。"""
    for line in text.splitlines():
        s = line.strip()
        for mark in ("///", "//!", "#", "//"):
            if s.startswith(mark):
                s = s[len(mark):].strip()
                if len(s) > 12:
                    return s.split(". ")[0].rstrip(".")
    for line in text.splitlines():
        if line.strip():
            return line.strip()[:120]
    return ""


def perturb_words(words: list[str], seed: str, drop_ratio: float) -> list[str]:
    """扰动：打乱词序 + 丢一部分词（丢多少由 seed 决定，确定性）。"""
    rng = random.Random(seed)
    kept = [w for w in words if rng.random() > drop_ratio]
    rng.shuffle(kept)
    return kept


def synergize(words: list[str]) -> list[str]:
    """同义替换（小词典；不命中的原样留下）。"""
    return [_SYNONYMS.get(w, w) for w in words]


def candidate_queries(docs: list[tuple[str, str]]) -> list[dict]:
    """产候选（未过门）：每文档最多 3 条（ident / sentence / combo）。"""
    by_id = dict(docs)
    ids = list(by_id)
    out: list[dict] = []
    for idx, (doc_id, text) in enumerate(docs):
        lang = "code" if doc_id.endswith((".rs", ".py")) else "text"
        syms = symbols(text, lang)
        if syms:
            words = protocol.split_identifier(syms[0])
            if len(words) >= 2:
                out.append({
                    "kind": "ident",
                    "text": " ".join(perturb_words(words, f"{doc_id}|ident", 0.34)),
                    "source": doc_id,
                })
        sent = first_sentence(text)
        if len(sent) > 20:
            words = protocol.tokenize(sent)
            if len(words) >= 4:
                out.append({
                    "kind": "sentence",
                    "text": " ".join(perturb_words(synergize(words), f"{doc_id}|sentence", 0.25)),
                    "source": doc_id,
                })
        if len(syms) >= 2 and len(ids) > 2:
            other_id = ids[(idx + len(ids) // 2) % len(ids)]
            other_syms = symbols(by_id[other_id], "code") or ["x"]
            words = protocol.split_identifier(syms[1]) + protocol.split_identifier(other_syms[0])
            if len(words) >= 2:
                out.append({
                    "kind": "combo",
                    "text": " ".join(perturb_words(words, f"{doc_id}|combo", 0.0)),
                    "source": doc_id,
                })
    return out


#: 分层抽样的目标规模（按 源×种类 轮转取，确定性）。
TARGET_QUERIES = 120


def _source_of(doc_id: str) -> str:
    for name, globs in SOURCES:
        for g in globs:
            prefix = g.split("*")[0].rstrip("/")
            if prefix and doc_id.startswith(prefix):
                return name
    return "other"


def sample(kept: list[dict], target: int) -> tuple[list[dict], dict]:
    """按 (源, 种类) 分层轮转抽样到 target 条——确定性：组内按原序、组间按名字排序。"""
    groups: dict[tuple[str, str], list[dict]] = {}
    for q in kept:
        groups.setdefault((_source_of(q["source"]), q["kind"]), []).append(q)
    order = sorted(groups)
    picked: list[dict] = []
    round_no = 0
    while len(picked) < target and any(groups[k] for k in order):
        for key in order:
            if len(picked) >= target:
                break
            bucket = groups[key]
            if round_no < len(bucket):
                picked.append(bucket[round_no])
        round_no += 1
    picked.sort(key=lambda q: (q["source"], q["kind"], q["text"]))
    counts: dict[str, int] = {}
    for q in picked:
        group_key = f"{_source_of(q['source'])}/{q['kind']}"
        counts[group_key] = counts.get(group_key, 0) + 1
    return picked, {"sampled": len(picked), "by_group": dict(sorted(counts.items())),
                    "bucket_sizes": {f"{k[0]}/{k[1]}": len(v) for k, v in sorted(groups.items())}}


def generate(docs: list[tuple[str, str]]) -> tuple[list[dict], dict]:
    """过门 → 分层抽样。返回 (冻结的查询集, 产率读数)。"""
    corpus_texts = [t for _id, t in docs]
    prepared = protocol.prepare(corpus_texts)          # 语料只切一次（见 protocol 里的性能说明）
    tokens_by_id = dict(zip([d for d, _t in docs], prepared[1]))
    kept: list[dict] = []
    dropped: dict[str, int] = {}
    for cand in candidate_queries(docs):
        ok, why = protocol.verdict_prepared(cand["text"], tokens_by_id[cand["source"]], prepared)
        if not ok:
            key = why[0].split(" ")[0]
            dropped[key] = dropped.get(key, 0) + 1
            continue
        kept.append(cand)
    picked, sampling = sample(kept, TARGET_QUERIES)
    queries = [{
        "qid": f"q{i:03d}",
        "text": q["text"],
        "kind": q["kind"],
        "source": q["source"],
        "relevant": [q["source"]],
    } for i, q in enumerate(picked)]
    stats = {
        "docs": len(docs),
        "candidates": len(kept) + sum(dropped.values()),
        "kept": len(kept),
        "dropped": dropped,
        "yield": round(len(kept) / max(1, len(kept) + sum(dropped.values())), 3),
        **sampling,
    }
    return queries, stats


def render_queries(queries: list[dict]) -> str:
    return "".join(json.dumps(q, ensure_ascii=False, sort_keys=True) + "\n" for q in queries)


def render_manifest(docs: list[tuple[str, str]]) -> str:
    lines = []
    for doc_id, text in docs:
        digest = hashlib.sha256(text.encode("utf-8")).hexdigest()
        lines.append(f"{digest}  {doc_id}\n")
    return "".join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true", help="写冻结的语料清单与查询集")
    ap.add_argument("--check", action="store_true", help="重放并与冻结文件逐字节对账")
    ap.add_argument("--stats", action="store_true", help="只打印规模与产率")
    a = ap.parse_args(argv)

    docs = enumerate_docs()
    queries, stats = generate(docs)
    if a.stats or not (a.write or a.check):
        print(json.dumps(stats, ensure_ascii=False, sort_keys=True))
        return 0
    manifest, body = render_manifest(docs), render_queries(queries)
    if a.write:
        CORPUS_MANIFEST.write_text(manifest, encoding="utf-8", newline="\n")
        QUERIES.write_text(body, encoding="utf-8", newline="\n")
        print(f"GEN-WRITE OK：{stats}")
        return 0
    ok = True
    for path, want in ((CORPUS_MANIFEST, manifest), (QUERIES, body)):
        got = path.read_text(encoding="utf-8") if path.is_file() else ""
        if got != want:
            print(f"GEN-CHECK FAIL：{path.name} 与重放结果不一致（改动必须显式 --write）")
            ok = False
    print(f"GEN-CHECK {'OK' if ok else 'FAIL'}：{stats}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
