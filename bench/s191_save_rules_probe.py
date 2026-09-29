"""S191 存档区两条规则的**误报抽样账**（判定档红线：启发式规则必须能量化精度）。

口径（不做假的自动判真伪）：
- **候选面** = 该语言面上"可能与规则相关"的锚点行（写锚点 / 反序列化锚点）；
- **命中面** = `bug_scan` 报出的 `save_*` 条目；
- 本文件只做**计数与并列**（命中行 + 该行原文），真伪由人读码填——自动判真伪
  会把自己的判据当结论，正是本仓反复防的"门自己骗自己"。

用法：
  python -X utf8 bench/s191_save_rules_probe.py [root]
默认 root = bench/manual_snaps（VoxelForge 快照语料，32 文件）。

纪律：本文件是**测量夹具不是扫描器**——规则唯一实现在 `rust/src/bug/save.rs`。
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)
os.environ.setdefault("UNIFIED_RX_SANDBOX", "*")  # 与 bench 其它脚本同纪律（S97）

import registry  # noqa: E402
import tools  # noqa: F401,E402

# 与 rust/src/bug/save.rs 的锚点表同口径（两份是"规则 vs 测量"，故意各自独立）
# 左边界：前一个字符不是词字符（`extend_from_slice` 里的 `from_slice` 不算锚点）
ANCHOR_RE = re.compile(
    r"(?<![A-Za-z0-9_])(fs::write|File::create|create_new|from_str|from_slice"
    r"|from_reader|from_bytes|deserialize|from_value)")


def candidates(root):
    """候选锚点行：(相对路径, 行号, 行原文)。注释行不算（与规则同口径）。"""
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in {"__pycache__", ".git", "target"}]
        for fn in sorted(filenames):
            if not fn.endswith(".rs"):
                continue
            fp = os.path.join(dirpath, fn)
            try:
                with open(fp, encoding="utf-8", errors="replace") as fh:
                    text = fh.read()
            except OSError:
                continue
            for i, line in enumerate(text.split("\n"), 1):
                if line.strip().startswith("//"):
                    continue
                if ANCHOR_RE.search(line):
                    out.append((os.path.relpath(fp, root), i, line.strip()))
    return out


def hits(root):
    r = registry.call("bug_scan", {"path": root, "max_files": 600})
    if not r.get("ok"):
        raise SystemExit(f"bug_scan 失败：{r.get('error')}")
    return [i for i in r["result"]["issues"] if i["rule"].startswith("save_")]


def main(argv):
    root = argv[0] if argv else os.path.join(HERE, "manual_snaps")
    cand = candidates(root)
    got = hits(root)
    print(f"ROOT {root}")
    print(f"候选锚点行 {len(cand)}（写/解析锚点，注释行已排除）")
    print(f"命中 {len(got)}（save_nonatomic_write "
          f"{sum(1 for i in got if i['rule'] == 'save_nonatomic_write')} / "
          f"save_load_no_version "
          f"{sum(1 for i in got if i['rule'] == 'save_load_no_version')}）")
    print("--- 命中明细（读码填真伪）---")
    for i in got:
        print(f"  [{i['severity']}] {i['file']}:{i['line']} {i['rule']}")
    print("--- 候选但未报（读码确认是否该报）---")
    fired = {(os.path.basename(i["file"]), i["line"]) for i in got}
    for rel, line, text in cand:
        if (os.path.basename(rel), line) not in fired:
            print(f"  {rel}:{line}  {text[:100]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
