"""naming_gate —— 命名纪律门（用户命名宪法：除主名外，一切命名必须专业）。

机器判据（三条，全部只看 **git 跟踪** 的文本文件——工作区杂物由 .gitignore 防误提交）：
  1. 禁占位/随意命名：foo/bar/baz、test123、untitled、asdf、aaa+、xxx+、placeholder、  # naming:allow（本门词表/用法示例，扫描器自指豁免）
     lorem ipsum 作为独立词出现在代码与文档（测试夹具/第三方引用路径由行级豁免标记放行）；  # naming:allow（本门词表/用法示例，扫描器自指豁免）
  2. 禁口语副名：主名白名单之外的「项目名/品牌名 + 口语括注」——用 --forbid 显式登记
     需要退役的旧名（如 PhysArena 改 BSHSQ 时登记 PhysArena/RUST WL，改名后一段时间内  # naming:allow（本门词表/用法示例，扫描器自指豁免）
     保持零命中，防止旧名从文档回流）；
  3. 禁杂物被跟踪：_patch_*.py、__pycache__/、*.pyc、.urx-hist-* 被跟踪即红。

行级豁免：行内含 `naming:allow`（或 `naming-allow`）标记的行跳过——豁免必须显式写进
源码，评审可见；没有豁免文件，没有静默白名单。

用法：
  python -X utf8 scripts/naming_gate.py --forbid PhysArena --forbid "RUST WL"  # naming:allow（本门词表/用法示例，扫描器自指豁免）
退出码：0 = 通过；1 = 命中；2 = 用法错误。
"""
import argparse
import re
import subprocess
import sys

# 占位/随意词（独立词匹配，大小写不敏感）；bar/baz 单独是正常英文词，只在 foo 三连里算。  # naming:allow（本门词表/用法示例，扫描器自指豁免）
PLACEHOLDER_WORDS = ["foo", "test123", "untitled", "asdf", "placeholder", "lorem ipsum"]  # naming:allow（本门词表/用法示例，扫描器自指豁免）
# aaa+/xxx+ 等重复字母串（≥3 连）单独算  # naming:allow（本门词表/用法示例，扫描器自指豁免）
REPEAT_RE = re.compile(r"\b(a{3,}|x{3,})\b", re.IGNORECASE)
FOO_BAR_RE = re.compile(r"\bfoo\b[^\n]*\bbar\b|\bbar\b[^\n]*\bbaz\b", re.IGNORECASE)
ALLOW_MARK = re.compile(r"naming[:_-]allow")

JUNK_TRACKED = re.compile(r"(^|/)(_patch_.*\.py|.*\.pyc|__pycache__/.*|\.urx-hist-[^/]*/.*)$")

SKIP_EXT = (".wasm", ".png", ".jpg", ".jpeg", ".gif", ".ico", ".woff", ".woff2",
            ".ttf", ".pdf", ".zip", ".gz", ".lock", ".sum")
SKIP_DIRS = ("node_modules/", "dist/", "vendor/", "third_party/", "testdata/",
             # 语料/产物：快照里是第三方游戏代码（AAA 是内容不是命名），results 是跑分产物  # naming:allow（本门词表/用法示例，扫描器自指豁免）
             "bench/manual_snaps", "bench/results",
             "bench/manual_review_pack.txt", "bench/vf3_baseline.json")


def tracked_files():
    out = subprocess.run(["git", "ls-files"], capture_output=True, text=True,
                         encoding="utf-8", errors="replace")
    if out.returncode != 0:
        sys.exit(f"git ls-files 失败: {out.stderr.strip()}")
    return [f for f in out.stdout.splitlines() if f]


def is_test_zone(path):
    """测试区天然含占位样例（与 secrets 门「测试区豁免」同哲学）：检测器必须先
    会写占位才能检测占位；夹具假值锁在测试面。产品代码的特例用行级 naming:allow。"""
    p = path.replace("\\", "/")
    base = p.rsplit("/", 1)[-1]
    return ("/tests/" in p or p.startswith("tests/") or base.startswith("test_")
            or base.endswith("_test.rs") or base == "conftest.py")


def build_rules(forbid):
    rules = [(re.compile(rf"\b{re.escape(w)}\b", re.IGNORECASE), w) for w in PLACEHOLDER_WORDS]
    rules.append((FOO_BAR_RE, "foo/bar|bar/baz 占位三连"))  # naming:allow（本门词表/用法示例，扫描器自指豁免）
    rules.append((REPEAT_RE, "aaa/xxx 重复字母串"))  # naming:allow（本门词表/用法示例，扫描器自指豁免）
    for old in forbid:
        # 退役旧名全仓扫（含测试区）——旧名不许从任何文档回流。
        # 独立词匹配：不误捕含旧名前缀的技术标识（unified-rx-rs 这类，由 --allow-name 记账）。
        rules += [
            (re.compile(rf"(?<![-\w]){re.escape(old)}(?![-\w])", re.IGNORECASE),
             f"退役旧名 {old!r}"),
        ]
    return rules


def scan_files(rules, allow_res):
    """扫全部跟踪文件，返回（违规清单, 白名单技术标识计数）。"""
    bad, allowed = [], 0
    for f in tracked_files():
        if f.endswith(SKIP_EXT) or any(f.startswith(d) or f"/{d}" in f for d in SKIP_DIRS):
            continue
        if JUNK_TRACKED.search(f):
            bad.append((f, 0, "(文件名)", "被跟踪的杂物（开发残留/缓存）"))
            continue
        try:
            with open(f, encoding="utf-8") as fh:
                text = fh.read()
        except (UnicodeDecodeError, OSError):
            continue
        for no, hit, label in iter_hit_lines(text, rules):
            # 测试区豁免只放行占位词；退役旧名在哪都红
            if not label.startswith("退役旧名") and is_test_zone(f):
                continue
            bad.append((f, no, hit, label))
        for ar in allow_res:
            allowed += len(ar.findall(text))
    return bad, allowed


def iter_hit_lines(text, rules):
    for no, line in enumerate(text.splitlines(), 1):
        if ALLOW_MARK.search(line):
            continue
        for pat, label in rules:
            m = pat.search(line)
            if m:
                yield no, m.group(0), label
                break


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--forbid", action="append", default=[],
                    help="退役旧名（口语副名），命中即红；可多次")
    ap.add_argument("--allow-name", action="append", default=[],
                    help="技术标识白名单（crate 名/缓存路径等含旧名前缀的实现层标识），可多次")
    args = ap.parse_args(argv)

    rules = build_rules(args.forbid)
    allow_res = [re.compile(re.escape(n)) for n in args.allow_name]
    bad, allowed = scan_files(rules, allow_res)

    if bad:
        print(f"NAMING-GATE FAIL 命中={len(bad)}（豁免标记 naming:allow 可放行特例）")
        for f, no, hit, label in bad[:25]:
            where = f"{f}:{no}" if no else f
            print(f"  ✗ {where}: {label} → {hit[:60]}")
        if len(bad) > 25:
            print(f"  …另有 {len(bad) - 25} 条")
        return 1
    note = f"；白名单技术标识 {allowed} 处在案" if allowed else ""
    print(f"NAMING-GATE OK（占位词/杂物/退役旧名零命中{note}）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
