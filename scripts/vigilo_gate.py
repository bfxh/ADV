#!/usr/bin/env python3
"""Vigilo 安全扫描增量门——基线棘轮，只拦新增发现。

Vigilo 是全仓扫描器（AST + 数据流），首次跑会报几百条存量。直接当门会全红，
所以用基线棘轮：记录当前发现集合，此后只报 **不在基线里的新发现**。

用法：
  python scripts/vigilo_gate.py                        # 门模式：新发现即红
  python scripts/vigilo_gate.py --write-baseline       # 记录/更新基线
  python scripts/vigilo_gate.py --list                 # 列出当前发现（不判红）
  python scripts/vigilo_gate.py --min-severity high    # 只看 high+

退出码：0 = 无新增；1 = 有新发现；2 = vigilo 未安装或用法错。
"""
import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_BASELINE = os.path.join(ROOT, "spec", "vigilo-baseline.json")


def _run_vigilo(min_severity: str) -> list[dict]:
    exe = shutil.which("vigilo")
    if not exe:
        print("VIGILO-GATE FAIL: vigilo 未安装（pip install vigilo）")
        sys.exit(2)
    cmd = [sys.executable, "-X", "utf8", exe, "scan", ".",
           "--format", "json", "--min-severity", min_severity]
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT, timeout=300)
    if r.returncode not in (0, 1):
        print(f"VIGILO-GATE FAIL: vigilo 异常退出 {r.returncode}\n{r.stderr[:500]}")
        sys.exit(2)
    try:
        return json.loads(r.stdout).get("findings", [])
    except json.JSONDecodeError as e:
        print(f"VIGILO-GATE FAIL: vigilo 输出不是合法 JSON（{e}）")
        sys.exit(2)


def _finding_key(f: dict) -> str:
    loc = f["location"]
    return f"{f['id']}:{loc['file']}:{loc['line']}:{loc['col']}"


def _rule_counts(findings: list[dict]) -> Counter:
    return Counter(f["id"] for f in findings)


def _load_baseline(path: str) -> set[str]:
    if not os.path.isfile(path):
        return set()
    try:
        data = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
        return set(data.get("keys", []))
    except (json.JSONDecodeError, KeyError) as e:
        print(f"VIGILO-GATE FAIL: 基线损坏（{e}）——重跑 --write-baseline 修复")
        sys.exit(2)


def _write_baseline(path: str, keys: set[str], findings: list[dict]) -> None:
    payload = {"keys": sorted(keys), "count": len(keys), "by_rule": dict(_rule_counts(findings))}
    tmp_fd, tmp_path = tempfile.mkstemp(dir=os.path.dirname(path), suffix=".tmp")
    try:
        with os.fdopen(tmp_fd, "w", encoding="utf-8") as fp:
            json.dump(payload, fp, ensure_ascii=False, indent=1)
            fp.write("\n")
        os.replace(tmp_path, path)
    except BaseException:
        os.unlink(tmp_path)
        raise


def _print_list(findings: list[dict]) -> None:
    for rid, cnt in _rule_counts(findings).most_common():
        print(f"  {rid}: {cnt}x")


def _report_new(new_findings: list[dict]) -> None:
    print(f"\n新增 {len(new_findings)} 条（不在基线中）：")
    for rid, cnt in _rule_counts(new_findings).most_common():
        for ex in [f for f in new_findings if f["id"] == rid][:3]:
            loc = ex["location"]
            print(f"  ✗ {rid} {loc['file']}:{loc['line']} — {ex['message']}")
        if cnt > 3:
            print(f"  …另有 {cnt - 3} 条 {rid}")


def main() -> int:
    ap = argparse.ArgumentParser(description="Vigilo 增量安全门")
    ap.add_argument("--baseline", default=DEFAULT_BASELINE)
    ap.add_argument("--write-baseline", action="store_true")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--min-severity", default="low", choices=["low", "medium", "high"])
    a = ap.parse_args()

    findings = _run_vigilo(a.min_severity)
    keys = {_finding_key(f) for f in findings}
    print(f"VIGILO-GATE 发现={len(findings)} 唯一键={len(keys)} "
          f"min-severity={a.min_severity} 基线={os.path.basename(a.baseline)}")

    if a.list:
        _print_list(findings)
        return 0

    if a.write_baseline:
        _write_baseline(a.baseline, keys, findings)
        print(f"已写基线 {a.baseline}（{len(keys)} 条）——此后只拦新增")
        return 0

    base = _load_baseline(a.baseline)
    if not base:
        print("警告：尚无基线 ⇒ 所有发现都算新增；先跑 --write-baseline 建立存量基线")

    new_findings = [f for f in findings if _finding_key(f) not in base]
    if new_findings:
        _report_new(new_findings)
        print(f"\nVIGILO-GATE FAIL 新增={len(new_findings)}")
        return 1

    gone = base - keys
    if gone:
        print(f"  （{len(gone)} 条基线发现已消失，可跑 --write-baseline 收紧）")
    print("VIGILO-GATE OK 无新增发现")
    return 0


if __name__ == "__main__":
    sys.exit(main())
