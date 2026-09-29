"""claim_gate.py —— 主张可复算门（S194，用户「减少幻觉」口径的第一片可判形式）。

**为什么只做这一半**：用户点名的五类表述幻觉（把过程变成东西 / 关系变属性 / 条件变本质 /
局部变总体 / 解释变终点）在**散文层不可靠地可判**——本片四轮测量的数字：字面词表版
`过程→东西` 0 处、`代理当目标` 0 处、`局部→总体` 106 处里绝大多数是真审计的正常表述；
`断言式普遍化` 修完口径后剩 8 处，**全是该绝对化的政策不变量**（"扫描对象永远是复制品"）。
⇒ **造词表门 = 造一个见谁都报的假门**（正是本仓 T7「仪器失效」要防的）。

可判的那一半是**结构**：**当时的观察被固化成文档里的事实**，然后随代码漂移——这类
`数字主张 ↔ 真值源` 的关系是机器能算的。本门就只做它：登记表里每条主张必须与真值逐位一致。

fail-closed：登记表缺失 / 坏 JSON / 空登记 / 条目缺字段 ⇒ 判红（门被清空即失效）；
**主张找不到也判红**（"账没了"与"账错了"同罪，同快照棘轮纪律）。

真值源四种（都在仓内、零依赖、每条都能被夹具替换）：
  `selftest:tools` / `selftest:groups` / `selftest:group:<域>` / `selftest:exe`
      跑 `python server.py --selftest`（**真路径**：与 CI 看的是同一行）取数字；
  `steps:total` / `steps:fast` / `steps:full`
      ast 解析 `scripts/local_gate.py` 的 `STEPS` 数档位条数（不 import，免副作用）；
  `count_re:<路径>:<正则>`  数文件里命中数（如 `#[test]`）；
  `json_len:<路径>:<点号指针>`  取 JSON 数组长度（如 `rules`）。

用法：
  python -X utf8 scripts/claim_gate.py                                  # 扫本仓（默认登记表）
  python -X utf8 scripts/claim_gate.py --root <dir> --claims <json>     # 夹具/判据用
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
STEPS_FILE = "scripts/local_gate.py"


def _selftest(root: pathlib.Path) -> str:
    """跑仓内 server.py --selftest（真路径）；非零退出即抛（不静默）。

    沙盒口径与 `local_gate._child_env` 的 selftest 步一致（`UNIFIED_RX_SANDBOX=ROOT`）：
    本门常被门链以某个夹具沙盒调起，不覆盖就会"路径越界（沙盒外）"——真值源要说得出话。
    """
    if not (root / "server.py").is_file():
        raise ValueError(f"真值源不可用：{root}/server.py 不存在")
    env = dict(os.environ)
    env["PYTHONUTF8"] = "1"
    env["UNIFIED_RX_SANDBOX"] = str(root)
    p = subprocess.run([sys.executable, "-X", "utf8", "server.py", "--selftest"],
                       cwd=str(root), env=env, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", shell=False, timeout=300)
    if p.returncode != 0:
        raise ValueError(f"selftest 退出码 {p.returncode}：{(p.stdout or '')[-200:]}")
    return p.stdout


def _steps_counts(root: pathlib.Path) -> dict[str, int]:
    """ast 解析 STEPS（不 import）：数总条数 / fast 档 / fast+full 档。"""
    src = (root / STEPS_FILE).read_text(encoding="utf-8", errors="replace")
    tree = ast.parse(src)
    node = next((n for n in ast.walk(tree)
                 if isinstance(n, ast.Assign)
                 and any(getattr(t, "id", "") == "STEPS" for t in n.targets)), None)
    if node is None or not isinstance(node.value, (ast.List, ast.Tuple)):
        raise ValueError(f"{STEPS_FILE} 里找不到 STEPS 列表——门链被改名？")
    tiers = []
    for el in node.value.elts:
        # 元组第二项是 argv 列表（含 PY/ROOT 等变量，literal_eval 会失败）——
        # 只读**第三项**：档位是字符串字面量。
        tier_node = el.elts[2] if isinstance(el, ast.Tuple) and len(el.elts) > 2 else None
        tiers.append(tier_node.value if isinstance(tier_node, ast.Constant)
                     and isinstance(tier_node.value, str) else "?")
    if "?" in tiers:
        raise ValueError(f"{STEPS_FILE} 有 {tiers.count('?')} 个条目读不出档位"
                         "——门链结构变了，先修真值源再谈文档（不许按 0 计数）")
    return {"total": len(tiers),
            "fast": sum(1 for t in tiers if t == "fast"),
            "full": sum(1 for t in tiers if t in ("fast", "full"))}


def _profile_count(root: pathlib.Path, name: str) -> int:
    """仓内**真口径**算档位件数：用 registry.profile_from_env() 的域集合 Σ groups()
    （不把域清单复制进门里——复制就会漂移）。"""
    snippet = (
        "import os\n"
        "os.environ.setdefault('UNIFIED_RX_SANDBOX', '__URX_UNSET__')\n"
        f"os.environ['UNIFIED_RX_PROFILE'] = {name!r}\n"
        "import registry, tools\n"
        "g = registry.profile_from_env() or set()\n"
        "print('PROFILE', sum(len(v) for k, v in registry.groups().items() if k in g))\n")
    p = subprocess.run([sys.executable, "-X", "utf8", "-c", snippet], cwd=str(root),
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", shell=False, timeout=300)
    m = re.search(r"PROFILE (\d+)", p.stdout or "")
    if p.returncode != 0 or not m:
        raise ValueError(f"profile:{name} 真值源失败 rc={p.returncode}："
                         f"{(p.stderr or '')[-200:]}")
    return int(m.group(1))


def _probe(kind: str, root: pathlib.Path) -> int:
    """按真值源名取一个整数真值；未知源即抛（fail-closed）。"""
    if kind.startswith("profile:"):
        return _profile_count(root, kind.split(":", 1)[1])
    if kind.startswith("selftest:"):
        key = kind.split(":", 1)[1]
        out = _selftest(root)
        pat = {"tools": r"SELFTEST tools=(\d+)", "groups": r"GROUPS (\d+):",
               "exe": r"EXE_TAG ok=(\d+)"}.get(key)
        if pat is None and key.startswith("group:"):
            name = key.split(":", 1)[1]
            pat = rf"\b{re.escape(name)}\((\d+)\)"
        if pat is None:
            raise ValueError(f"未知 selftest 真值源: {kind}")
        m = re.search(pat, out)
        if not m:
            raise ValueError(f"selftest 输出里找不到 {kind}（行格式变了？）")
        return int(m.group(1))
    if kind.startswith("steps:"):
        return _steps_counts(root)[kind.split(":", 1)[1]]
    if kind.startswith("count_re:"):
        _, rel, pat = kind.split(":", 2)
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        return len(re.findall(pat, text))
    if kind.startswith("json_len:"):
        _, rel, ptr = kind.split(":", 2)
        data = json.loads((root / rel).read_text(encoding="utf-8"))
        cur = data
        for part in [p for p in ptr.split(".") if p]:
            cur = cur[part]
        return len(cur)
    raise ValueError(f"未知真值源: {kind}")


def load_claims(path: pathlib.Path) -> list[dict]:
    """登记表 → 条目列表；**任何不健康都抛 ValueError**（fail-closed 由调用方判红）。"""
    if not path.is_file():
        raise ValueError(f"登记表不存在: {path}")
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ValueError(f"登记表不可读/坏 JSON: {exc}") from exc
    claims = doc.get("claims") if isinstance(doc, dict) else None
    if not isinstance(claims, list) or not claims:
        raise ValueError("claims 必须是非空数组——门被清空即失效（占位即红）")
    for c in claims:
        if not isinstance(c, dict) or not c.get("id") or not c.get("file") \
                or not c.get("pattern") or not isinstance(c.get("expect"), list) \
                or not c["expect"]:
            raise ValueError(f"条目缺字段（id/file/pattern/expect 必填）: {c!r}")
        for kind in c["expect"]:
            if not isinstance(kind, str) or ":" not in kind:
                raise ValueError(f"{c['id']} 的 expect 项格式错（需 `<源>:<参>`）: {kind!r}")
    return claims


def load_entry_contract(path: pathlib.Path) -> dict:
    """账目格式契约（第二族判据）：`entry_contract` = {file, require[], min_session}。

    治的是「**把解释变成终点**」的结构等价物——条目只写归因、不附证据/决策。
    口径是**棘轮钉现状**：`min_session` 之前的条目祖父化（历史不追溯），之后必须齐栏。
    """
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ValueError(f"登记表不可读/坏 JSON: {exc}") from exc
    ec = doc.get("entry_contract") if isinstance(doc, dict) else None
    if not isinstance(ec, dict) or not ec.get("file") \
            or not isinstance(ec.get("require"), list) or not ec["require"] \
            or not isinstance(ec.get("min_session"), int):
        raise ValueError("entry_contract 缺失/字段不全（file/require[]/min_session 必填）"
                         "——账目契约被清空即失效（占位即红）")
    return ec


def entry_violations(root: pathlib.Path, ec: dict) -> list[str]:
    """按契约扫账目：`## S<编号> ·` 分块，编号 ≥ min_session 的条目必须齐 `require` 各栏。"""
    fp = root / ec["file"]
    if not fp.is_file():
        return [f"账目不在（{ec['file']}）——契约指向空气？"]
    blocks = re.split(r"\n(?=## S\d+ · )", fp.read_text(encoding="utf-8", errors="replace"))
    entries = [b for b in blocks if b.startswith("## S")]
    if not entries:
        return [f"{ec['file']} 里一条 `## S<编号> ·` 都没有——账没了（读完即失效）"]
    bad: list[str] = []
    for b in entries:
        m = re.match(r"## S(\d+) ·", b)
        if not m or int(m.group(1)) < ec["min_session"]:
            continue
        missing = [f for f in ec["require"] if f not in b]
        if missing:
            bad.append(f"{ec['file']} S{m.group(1)}: 缺 {' / '.join(missing)}"
                       f"（≥S{ec['min_session']} 的条目必须齐栏）")
    return bad


def violations(root: pathlib.Path, claims: list[dict]) -> list[str]:
    """逐条复算 → 不一致/找不到都记一条（含真值，便于一眼对账）。"""
    bad: list[str] = []
    for c in claims:
        fp = root / c["file"]
        if not fp.is_file():
            bad.append(f"{c['id']}: 文档不在（{c['file']}）——登记表指向空气？")
            continue
        m = re.search(c["pattern"], fp.read_text(encoding="utf-8", errors="replace"))
        if not m:
            bad.append(f"{c['id']}: 主张找不到（{c['file']} 里 `{c['pattern'][:40]}` 无命中）"
                       "——文案改了就该同步登记表")
            continue
        got = [int(g) for g in m.groups() if g is not None]
        try:
            want = [_probe(k, root) for k in c["expect"]]
        except (ValueError, KeyError) as e:
            bad.append(f"{c['id']}: 真值源失败——{e}")
            continue
        if got != want:
            bad.append(f"{c['id']}（{c['file']}）: 主张 {got} ≠ 真值 {want}"
                       f"（源 {c['expect']}）")
    return bad


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    ap.add_argument("--claims", default="")
    a = ap.parse_args(argv)
    root = pathlib.Path(a.root).resolve()
    cpath = pathlib.Path(a.claims).resolve() if a.claims else root / "spec" / "claim-checks.json"
    try:
        claims = load_claims(cpath)
        ec = load_entry_contract(cpath)
    except ValueError as exc:
        print(f"CLAIM-GATE FAIL 登记表不健康：{exc}")
        return 1
    bad = violations(root, claims) + entry_violations(root, ec)
    for line in bad:
        print(f"  ✗ {line}")
    print(f"CLAIM-GATE {'OK' if not bad else 'FAIL'} claims={len(claims)} "
          f"entry>={ec['min_session']} 栏位={len(ec['require'])} mismatch={len(bad)} "
          f"登记表={cpath.name}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
