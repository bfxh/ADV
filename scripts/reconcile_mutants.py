"""变异门同轮对账（`xtask mutants` 的披露通道，人读口径统一在这里）。

为什么要有它（2026-10-06 实测逼出来的）：`xtask mutants` 只判"新增未捕获 = 红"，
而一片做完时要回答的是四个不同的问题，我此前每次都在 outcomes.json 上手写 python 复算——
复算不是判据，而且会出错（同一天我就把"自己复算的数"当成门的结论记进过账本）。

四份清单，逐键列名，一个都不能省：
  · 新债      = 本轮 missed − 基线；
  · 可划账    = 基线 − 本轮 missed，且本轮必须有 CaughtMutant（证明是**真杀掉**）；
  · 只有超时  = 基线键本轮全判 Timeout——测试把它挂住了，但没有 caught 记账，不许算划账；
  · 不可验证  = 剩下的在册键，逐条注明原因（"全 Unviable／无判定" vs "本轮没出现该键"）；
  · 盘满缺测  = unviable 条目日志里命中盘满签名（缺测不许读成"没问题"）。

`--keys a,b`：确认这几个键本轮真被杀（绿 ≠ 新代码被杀，必须单独核）。

退出码：0 = 无新债、无不可验证、无盘满缺测，且 --keys 全部被杀；2 = 产物或基线缺失
（**不允许把"没有基线"读成"没有债"**）；1 = 有上述任一项。

本脚本只做披露与核对，不替代 `xtask mutants` 的判红。
"""

import argparse
import json
import pathlib
import sys

VERIFIABLE = {"CaughtMutant", "MissedMutant", "Timeout"}
DISK_SIGNATURES = [
    "No space left on device",
    "final link failed",
    "ENOSPC",
    "os error 112",
    "not enough space",
    "磁盘空间不足",
]
OUTCOMES_DEFAULT = "target/mutants-out/mutants.out/outcomes.json"
BASELINE_DEFAULT = "tools/baselines/mutants-baseline.json"


def outcome_key(o: dict):
    """债键 `<源文件>::<函数名>`，取法与 xtask 的 `outcome_key` 一致。"""
    sc = o.get("scenario")
    m = sc.get("Mutant") if isinstance(sc, dict) else None
    if not isinstance(m, dict):
        return None
    fn = m.get("function") or {}
    file, name = m.get("file"), fn.get("function_name")
    return f"{file}::{name}" if file and name else None


def per_key_counts(outcomes: list) -> dict:
    """每个键的判定计数与是否可验证（可验证 = 至少一条落在 VERIFIABLE 里）。"""
    counts: dict = {}
    for o in outcomes:
        k = outcome_key(o)
        if not k:
            continue
        c = counts.setdefault(k, {})
        c[o.get("summary", "?")] = c.get(o.get("summary", "?"), 0) + 1
    return counts


def disk_hits(out_dir: pathlib.Path, outcomes: list) -> list:
    """unviable 条目里命中盘满签名的键。"""
    hits = []
    for o in outcomes:
        if o.get("summary") != "Unviable":
            continue
        rel = o.get("log_path")
        if not rel:
            continue
        log = out_dir / rel
        if not log.is_file():
            continue
        text = log.read_text(encoding="utf-8", errors="replace")
        sig = next((s for s in DISK_SIGNATURES if s in text), None)
        if sig:
            hits.append((outcome_key(o) or str(rel), sig, rel))
    return hits


def reconcile(outcomes: list, baseline: list) -> dict:
    """纯对账：返回四份清单与总数（便于单测直接喂数据）。

    在册键本轮不再 missed 的三种情形**不许混为一谈**（这一版是被反例逼出来的）：
      · 有 CaughtMutant 且无 MissedMutant ⇒ 真被杀，可划账；
      · 只有 Timeout ⇒ 测试把它挂住了（会拒绝），但没有 caught 记账——单列，不许当划账；
      · 本轮根本没有该键 ⇒ 没测到，更不能当划账。
    """
    counts = per_key_counts(outcomes)
    missed = sorted(k for k, c in counts.items() if c.get("MissedMutant"))
    base = set(baseline)
    new_debt = sorted(set(missed) - base)
    retired = sorted(base - set(missed))

    def has(k, name):
        return bool(counts.get(k, {}).get(name))

    payable = [k for k in retired if has(k, "CaughtMutant") and not has(k, "MissedMutant")]
    timeout_only = [
        k for k in retired if k not in payable and has(k, "Timeout") and not has(k, "CaughtMutant")
    ]
    # 剩下的在册键都算"本轮没拿到能支撑划账的判定"，但原因要分开写：
    # 全 Unviable（跑到了但没编译/没运行过判定）与本轮整条没出现是两种不同的修法。
    unverifiable = []
    for k in retired:
        if k in payable or k in timeout_only:
            continue
        reason = "本轮没出现该键" if k not in counts else "全 Unviable／无判定"
        unverifiable.append(f"{k}（{reason}）")
    return {
        "total": len(outcomes),
        "missed_keys": missed,
        "new_debt": new_debt,
        "payable": payable,
        "timeout_only": timeout_only,
        "unverifiable": sorted(unverifiable),
    }


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--outcomes", default=OUTCOMES_DEFAULT)
    ap.add_argument("--baseline", default=BASELINE_DEFAULT)
    ap.add_argument("--keys", default="", help="逗号分隔：确认这些键本轮真被杀")
    a = ap.parse_args(argv)

    root = pathlib.Path(a.root).resolve()
    opath = root / a.outcomes
    bpath = root / a.baseline
    if not opath.is_file():
        print(f"RECONCILE FAIL: 缺本轮产物 {a.outcomes}（先跑 xtask mutants；没有产物不等于没债）")
        return 2
    if not bpath.is_file():
        print(f"RECONCILE FAIL: 缺基线 {a.baseline}（缺基线 ≠ 没有债）")
        return 2

    outcomes = json.loads(opath.read_text(encoding="utf-8")).get("outcomes", [])
    baseline = json.loads(bpath.read_text(encoding="utf-8")).get("missed", [])
    r = reconcile(outcomes, baseline)
    hits = disk_hits(opath.parent, outcomes)

    print(f"对账 root={root} 产物条数={r['total']} 基线={len(baseline)} 键，本轮 missed={len(r['missed_keys'])} 键")
    for label, keys in (
        ("新债", r["new_debt"]),
        ("可划账（真被杀）", r["payable"]),
        ("只有超时（会拒绝，但没 caught 记账——别当划账）", r["timeout_only"]),
        ("不可验证（不许当划账）", r["unverifiable"]),
    ):
        print(f"  {label}：{len(keys)}")
        for k in keys:
            print(f"    - {k}")
    if hits:
        print(f"  盘满缺测：{len(hits)} 条")
        for k, sig, rel in hits[:20]:
            print(f"    - {k} ← {sig}（{rel}）")

    bad = bool(r["new_debt"] or r["unverifiable"] or r["timeout_only"] or hits)
    if a.keys:
        want = [k.strip() for k in a.keys.split(",") if k.strip()]
        counts = per_key_counts(outcomes)
        for k in want:
            c = counts.get(k, {})
            killed = bool(c.get("CaughtMutant")) and not c.get("MissedMutant")
            print(f"  核查 {k}: {'真被杀' if killed else '未杀/没测到'} 计数={c or '（无）'}")
            bad = bad or not killed
    print("RECONCILE OK" if not bad else "RECONCILE FAIL（见上面逐键清单）")
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main())
