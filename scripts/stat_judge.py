"""stat_judge.py —— 统计判定（S182，spec/FRONTIER-CI.md 方向⑩）：让性能判据从"单点比阈值"
升级为"**点估计 + 置信区间 + 三态结论**"。

**为什么必须这么做（本仓实测证据，2026-09-29 独占机器 7 轮采样）**：
现行 `perf_gate` 用"3 轮交错 A/B 取 **min**"得到一个比值，再与固定阈值比——实测同一机器上
单轮比值的极差 0.15–0.54，且**三次采到 ≥0.95 的样本**（secrets 1.001 / search 0.998 /
semantic 1.272，机器独占）⇒ 单点口径**必然假红**；而中位数稳定（0.53–0.86）。
⇒ 口径改为：**K 轮采样 → 中位数（点估计）+ Bootstrap 置信区间 → 三态判定**。

三态（**不把噪声当结论**）：
- `pass`：区间上界 ≤ limit（有把握不超阈）；
- `fail`：点估计 > limit **且**区间下界 > limit（有把握超阈——真回归）；
- `inconclusive`：区间跨阈（噪声内无法区分）——**报告真相，不硬判**；
  是否把 inconclusive 当红由 `PERF_STRICT=1` 决定（默认放行，避免噪声假红）。

确定性：Bootstrap **播种**（seed 由样本派生）⇒ 同输入同输出，可复现（本仓纪律）。
"""
from __future__ import annotations

import hashlib
import random
import statistics

DEFAULT_BOOT = 2000
DEFAULT_ALPHA = 0.05
MIN_N = 5


def _seed_of(samples: list[float], extra: str = "") -> int:
    """由样本派生种子：同输入同输出（跨进程可复现，不依赖全局随机态）。"""
    blob = "|".join(f"{x:.9f}" for x in samples) + "|" + extra
    return int.from_bytes(hashlib.sha256(blob.encode()).digest()[:4], "big")


def bootstrap_ci(samples: list[float], n_boot: int = DEFAULT_BOOT,
                 alpha: float = DEFAULT_ALPHA) -> tuple[float, float]:
    """中位数的 Bootstrap 置信区间（纯 stdlib：random.choices + 排序取分位）。

    **关于随机源**（回应静态检查的"弱随机数"提示）：此处 `random.Random` 是**正确选择**——
    用途是统计重采样（**非加密**），且**必须可播种复现**（"同输入同输出"是本仓纪律；
    `secrets` 无法给确定性序列）。不适用场景不套用加密随机源。
    """
    rnd = random.Random(_seed_of(samples, f"boot{n_boot}a{alpha}"))
    n = len(samples)
    meds = [statistics.median(rnd.choices(samples, k=n)) for _ in range(n_boot)]
    meds.sort()
    lo = meds[max(0, int(n_boot * alpha / 2))]
    hi = meds[min(n_boot - 1, int(n_boot * (1 - alpha / 2)) - 1)]
    return lo, hi


def judge(samples: list[float], limit: float, alpha: float = DEFAULT_ALPHA,
          n_boot: int = DEFAULT_BOOT, min_n: int = MIN_N) -> dict:
    """三态判定；样本不足时如实 `inconclusive` + `weak`（不假装判过）。"""
    if not samples:
        return {"verdict": "inconclusive", "median": None, "ci": None, "n": 0,
                "weak": True, "limit": limit, "reason": "无样本"}
    point = statistics.median(samples)
    n = len(samples)
    if n < min_n:
        return {"verdict": "inconclusive", "median": round(point, 4), "ci": None, "n": n,
                "weak": True, "limit": limit,
                "reason": f"样本 {n} < {min_n}：区间不可靠（如实标注，不假装判过）"}
    lo, hi = bootstrap_ci(samples, n_boot, alpha)
    if point > limit and lo > limit:
        verdict = "fail"
    elif hi <= limit:
        verdict = "pass"
    else:
        verdict = "inconclusive"
    return {"verdict": verdict, "median": round(point, 4),
            "ci": [round(lo, 4), round(hi, 4)], "n": n, "weak": False, "limit": limit,
            "min": round(min(samples), 4), "max": round(max(samples), 4),
            "spread": round(max(samples) - min(samples), 4),
            "margin": round(limit - point, 4), "reason": None}
