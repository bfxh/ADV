"""shard_plan.py —— 测试分片计划器（S186）：把"按文件名轮询"升级为"按耗时权重均衡"。

**动机（实测，2026-09-29 量 CI jobs 各步）**：pytest 分片按文件名轮询切 ⇒ 3.14 两分片
**256s vs 57s（4.5× 不均）**，而墙钟由最慢分片支配 ⇒ 白等约 100s。
CI 六个 job 的耗时表见 spec/ROUNDLOG.md S186。

**做法**：`--collect` 跑一次 `pytest --durations=0` 把**每文件耗时**落
`spec/pytest-durations.json`（权重表）；`--plan N --shard K` 用**最长优先贪心（LPT）**
分配（权重缺失的文件按中位数估），并**断言覆盖等价**（并集 == 全部 test_*.py、无重复）
——分片改动绝不允许丢测试。

用法：
  python -X utf8 scripts/shard_plan.py --collect              # 采样权重（跑全量 pytest）
  python -X utf8 scripts/shard_plan.py --plan 2 --shard 1     # 打印该分片的文件列表
  python -X utf8 scripts/shard_plan.py --verify 2             # 覆盖等价 + 预估不均衡 ≤ 25%
"""
from __future__ import annotations

import json
import pathlib
import re
import statistics
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
WEIGHTS = ROOT / "spec" / "pytest-durations.json"
_DUR_RE = re.compile(r"^\s*([\d.]+)s\s+\w+\s+(tests/[^:]+)::")
_MAX_IMBALANCE = 0.25          # 预估最慢/最快分片差超过此比例 ⇒ verify 红


def test_files() -> list[str]:
    return sorted(p.as_posix() for p in (ROOT / "tests").glob("test_*.py"))


def load_weights() -> dict:
    if not WEIGHTS.is_file():
        return {}
    try:
        doc = json.loads(WEIGHTS.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    per_file = doc.get("per_file_s") if isinstance(doc, dict) else None
    return per_file if isinstance(per_file, dict) else {}


def plan(files: list[str], weights: dict, n: int) -> list[list[str]]:
    """最长优先贪心（LPT）：确定性强、无需外部依赖；缺权重的文件按中位数估。"""
    default = statistics.median(weights.values()) if weights else 1.0
    buckets: list[list[str]] = [[] for _ in range(max(1, n))]
    load = [0.0] * len(buckets)
    for f in sorted(files, key=lambda x: (-float(weights.get(x, default)), x)):
        i = load.index(min(load))
        buckets[i].append(f)
        load[i] += float(weights.get(f, default))
    for b in buckets:
        b.sort()
    return buckets


def verify(files: list[str], buckets: list[list[str]]) -> tuple[bool, str]:
    """覆盖等价（并集=全集、无重复）+ 预估不均衡。**分片改动绝不许丢测试。**"""
    flat = [f for b in buckets for f in b]
    if sorted(flat) != sorted(files):
        missing = sorted(set(files) - set(flat))
        extra = sorted(set(flat) - set(files))
        return False, f"覆盖不等价：missing={missing[:3]} extra={extra[:3]}"
    if len(flat) != len(set(flat)):
        return False, "存在重复分片项"
    weights = load_weights()
    default = statistics.median(weights.values()) if weights else 1.0
    est = [sum(float(weights.get(f, default)) for f in b) for b in buckets]
    if len(est) > 1 and max(est) > 0:
        imb = (max(est) - min(est)) / max(est)
        if imb > _MAX_IMBALANCE:
            return False, (f"预估不均衡 {imb:.0%} > {_MAX_IMBALANCE:.0%}"
                           f"（分片耗时 {[round(x, 1) for x in est]}s）")
        return True, f"覆盖等价 ✓ 预估 {[round(x, 1) for x in est]}s 不均衡 {imb:.0%}"
    return True, "覆盖等价 ✓（单分片）"


def collect() -> int:
    """跑全量 pytest 采样每文件耗时 → spec/pytest-durations.json。"""
    t0 = time.time()
    cp = subprocess.run([sys.executable, "-X", "utf8", "-m", "pytest", "tests/", "-q",
                         "--durations=0", "-p", "no:cacheprovider"],
                        cwd=str(ROOT), capture_output=True, text=True, encoding="utf-8",
                        errors="replace", timeout=3600)
    per_file: dict[str, float] = {}
    for ln in (cp.stdout or "").splitlines():
        m = _DUR_RE.match(ln)
        if m:
            per_file[m.group(2)] = per_file.get(m.group(2), 0.0) + float(m.group(1))
    if not per_file:
        print("SHARD-PLAN FAIL: 未解析到 durations（pytest 输出格式变了？）")
        return 1
    doc = {"_doc": "每文件测试耗时采样（scripts/shard_plan.py --collect 生成；"
                   "分片计划用它做权重——权重漂移只会让均衡略变差，不会丢测试）",
           "generated_at": int(time.time()), "elapsed_s": round(time.time() - t0, 1),
           "pytest_rc": cp.returncode, "per_file_s": dict(sorted(per_file.items()))}
    WEIGHTS.parent.mkdir(parents=True, exist_ok=True)
    WEIGHTS.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"SHARD-PLAN collected: {len(per_file)} 文件（pytest rc={cp.returncode}，"
          f"{doc['elapsed_s']}s）→ {WEIGHTS}")
    return 0


def main(argv: list[str]) -> int:
    if "--collect" in argv:
        return collect()
    n = int(argv[argv.index("--plan") + 1]) if "--plan" in argv else 2
    files = test_files()
    buckets = plan(files, load_weights(), n)
    ok, detail = verify(files, buckets)
    if "--verify" in argv:
        print(f"SHARD-PLAN {'OK' if ok else 'FAIL'}: {detail}")
        return 0 if ok else 1
    if not ok:
        print(f"SHARD-PLAN FAIL: {detail}")
        return 1
    shard = int(argv[argv.index("--shard") + 1]) if "--shard" in argv else 0
    if not (1 <= shard <= n):
        print(f"SHARD-PLAN FAIL: --shard 需在 1..{n}")
        return 1
    print(" ".join(buckets[shard - 1]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
