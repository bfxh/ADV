"""stat_judge.py —— 统计**判定口径**（S182 建立 / S187 深化，spec/FRONTIER-CI.md 方向⑩）。

把性能判据从"单点比阈值"升级为"**点估计 + 置信区间 + 三态结论**"。

**为什么必须这么做**（本仓实测，2026-09-29 独占机器 7 轮）：旧口径"3 轮取 min 的比值 vs 固定
阈值"，单轮比值极差 **0.15–0.54**、三次采到 ≥0.95（secrets 1.001 / search 0.998 /
semantic 1.272）⇒ **必然假红**；中位数稳定（0.53–0.86）。完整证据见判定档 §⑩。

**三态**（不把噪声当结论）：`pass` 区间上界 ≤ limit；`fail` 点估计**与**区间下界双超限；
`inconclusive` 区间跨阈——报告真相，是否计红由 `PERF_STRICT=1` 决定。

**估计量与本模块分家**（S187 深化）：区间 / p 值 / Holm 校正住在 `stat_boot`，**族级**判定住在
`stat_family`；本模块只管**单项**的三态。**分辨力**（区间半宽）随结论一起报：
`pass` 只说明"没测到超过分辨力的回归"，**不等于"没有回归"**。

**默认档仍是百分位**（`method="percentile"`）——这是**量出来的决定**，不是因循：BCa 在六个
格子（Exp/N/对数正态 × n=15/25，400 次蒙卡）上区间**更窄**（如对数正态 n=15：1.269 vs 1.458）
但**覆盖系统性不足**（0.905–0.935 vs 0.925–0.958，名义 0.95）。本仓判据的立场是"**不把噪声
当结论**"⇒ 保守的那一档才是对的默认；BCa 作为可选档保留（要更窄的区间时显式选，并附带
"覆盖略低"的代价）。判据见 `tests/test_s187_stat_depth.py`（把这张表的**关系**钉住）。

确定性：Bootstrap **播种**（seed 由样本派生）⇒ 同输入同输出、跨进程可复现（本仓纪律）。
"""
from __future__ import annotations

import statistics

from stat_boot import DEFAULT_ALPHA, DEFAULT_BOOT, ci, median_dist, percentile_ci

MIN_N = 5


def bootstrap_ci(samples: list[float], n_boot: int = DEFAULT_BOOT,
                 alpha: float = DEFAULT_ALPHA) -> tuple[float, float]:
    """中位数的 Bootstrap **百分位**区间（既有口径**逐位保留**；BCa 见 `judge` / `stat_boot`）。"""
    return percentile_ci(median_dist(samples, n_boot, f"a{alpha}"), alpha)


def _unjudgeable(limit: float, n: int, point: float | None, reason: str) -> dict:
    """判不了就如实说（不假装判过）：`weak` 是**诚实标注**，不是"没问题"。"""
    return {"verdict": "inconclusive", "median": None if point is None else round(point, 4),
            "ci": None, "ci_method": None, "ci_note": None, "n": n, "weak": True,
            "limit": limit, "resolution": None, "mde_ratio": None, "reason": reason}


def _verdict(point: float, lo: float, hi: float, limit: float) -> str:
    """三态：点估计与区间下界**双双**超限 ⇒ fail；区间上界 ≤ 限 ⇒ pass；跨阈 ⇒ inconclusive。"""
    if point > limit and lo > limit:
        return "fail"
    return "pass" if hi <= limit else "inconclusive"


def _report(point: float, got: tuple[float, float, str, str | None], n: int,
            samples: list[float], limit: float) -> dict:
    """结果组装（结论 + 区间 + **分辨力** + 极值）——只此一处，免得各分支口径漂移。"""
    lo, hi, used, note = got
    half = (hi - lo) / 2
    return {"verdict": _verdict(point, lo, hi, limit), "median": round(point, 4),
            "ci": [round(lo, 4), round(hi, 4)], "ci_method": used, "ci_note": note,
            "n": n, "weak": False, "limit": limit,
            "min": round(min(samples), 4), "max": round(max(samples), 4),
            "spread": round(max(samples) - min(samples), 4),
            "resolution": round(half, 4),
            "mde_ratio": round(1 + half / limit, 4) if limit else None,
            "margin": round(limit - point, 4), "reason": None}


def judge(samples: list[float], limit: float, alpha: float = DEFAULT_ALPHA,
          n_boot: int = DEFAULT_BOOT, min_n: int = MIN_N,
          method: str = "percentile") -> dict:
    """单项三态判定 + **分辨力**；样本不足时如实 `weak`（不假装判过）。

    `resolution` = 区间半宽 = 这个样本量能测出的最小超阈量；`mde_ratio` 是同口径的比值形态。
    `method`：`percentile`（默认，保守、覆盖更接近名义）/ `bca`（更窄但覆盖略低，见模块 docstring）。
    """
    if not samples:
        return _unjudgeable(limit, 0, None, "无样本")
    point, n = statistics.median(samples), len(samples)
    if n < min_n:
        return _unjudgeable(limit, n, point, f"样本 {n} < {min_n}：区间不可靠（如实标注，不假装判过）")
    dist = median_dist(samples, n_boot, f"a{alpha}")
    return _report(point, ci(dist, samples, alpha, method), n, samples, limit)
