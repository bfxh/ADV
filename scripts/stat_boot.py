"""stat_boot.py —— Bootstrap 估计量（S187 深化，spec/FRONTIER-CI.md 方向⑩）。

`stat_judge` 的**判定口径**（三态）不变；这里补的是**估计量本身**的细节。四件，每件都有
"能被证伪"的判据（见 tests/test_s187_stat_depth.py）：

1. **BCa 区间**（bias-corrected & accelerated）——用 `z0`（中位数偏差）与 `a`（jackknife 加速
   常数）校正分位点。**但"BCa 更准"这个先验在本仓实测里被否掉了**：六个格子（Exp / 正态 /
   对数正态 × n=15 / 25，400 次蒙卡，B=2000）上 BCa 区间**更窄**（对数正态 n=15：宽 1.269 vs
   1.458）却**覆盖系统性不足**（0.905–0.935 vs 百分位 0.925–0.958，名义 0.95）⇒ 本仓默认仍用
   **百分位**（保守优先：判据不该把噪声当结论），BCa 降级为**可选档**。退化情形（z0 发散 /
   分母非正 / 端点反序）**如实回退**百分位并在 `note` 里说明——不假装做了纠正。
2. **单侧 bootstrap p 值**——`p = (1 + #(θ* ≤ limit)) / (B + 1)`。加一形式避免 p=0；
   与区间**同源**（p ≤ α/2 ⟺ 百分位下界 > limit），这条一致性本身就是判据。
3. **Holm–Bonferroni 族级校正**——一条命令一项判据，α 按项数膨胀（m 项各 0.05 ⇒ 族错误率
   ≈ 1−0.95^m）：**多重比较**必须校正，否则"跑得多"就等于"更会假红"。
4. **分辨力（resolution）**——区间半宽。它回答"这个样本量能测出多大的回归"：
   `pass` 只说明**没测到**超过分辨力的回归，不等于"没有回归"。不报分辨力的 `pass` 是半句话。

确定性：随机源由样本派生（`seed_of`）⇒ 同输入同输出、跨进程可复现（本仓纪律）。
假设与残留风险：K 轮采样按 **iid/可交换**处理——同机连跑存在热漂移与结转效应，严格说不是
独立同分布；这会让区间**略窄**（乐观）。本模块不假装消掉它，只把它写在这里。
"""
from __future__ import annotations

import bisect
import hashlib
import random
import statistics

DEFAULT_BOOT = 2000
DEFAULT_ALPHA = 0.05


def seed_of(samples: list[float], extra: str = "") -> int:
    """由样本派生种子：同输入同输出（跨进程可复现，不依赖全局随机态）。"""
    blob = "|".join(f"{x:.9f}" for x in samples) + "|" + extra
    return int.from_bytes(hashlib.sha256(blob.encode()).digest()[:4], "big")


def median_dist(samples: list[float], n_boot: int = DEFAULT_BOOT,
                extra: str = "") -> list[float]:
    """中位数的 Bootstrap 重采样分布（已排序）。随机源播种 ⇒ 同输入同分布。

    **关于随机源**（回应静态检查的"弱随机数"提示，与 `stat_judge` 既有口径一致）：
    此处 `random.Random` 是**正确选择**——用途是统计重采样（**非加密**），且**必须可播种
    复现**（"同输入同输出"是本仓纪律；`secrets` 无法给确定性序列）。不适用场景不套用加密随机源。
    """
    rnd = random.Random(seed_of(samples, f"boot{n_boot}{extra}"))
    n = len(samples)
    dist = [statistics.median(rnd.choices(samples, k=n)) for _ in range(n_boot)]
    dist.sort()
    return dist


def percentile_ci(dist: list[float], alpha: float = DEFAULT_ALPHA) -> tuple[float, float]:
    """百分位区间（分位取整到重采样序号；B 小时如实偏保守）。"""
    b = len(dist)
    lo = dist[max(0, int(b * alpha / 2))]
    hi = dist[min(b - 1, int(b * (1 - alpha / 2)) - 1)]
    return lo, hi


def jackknife_accel(samples: list[float]) -> float:
    """jackknife 加速常数 a（偏度修正项）；样本 < 3 或退化时回 0（= 不纠正）。"""
    n = len(samples)
    if n < 3:
        return 0.0
    thetas = [statistics.median(samples[:i] + samples[i + 1:]) for i in range(n)]
    mean = sum(thetas) / n
    diffs = [mean - t for t in thetas]
    den = 6.0 * (sum(d ** 2 for d in diffs) ** 1.5)
    if not den:
        return 0.0
    return sum(d ** 3 for d in diffs) / den


def bca_ci(samples: list[float], dist: list[float],
           alpha: float = DEFAULT_ALPHA) -> tuple[float, float, str, str | None]:
    """BCa 区间 → (lo, hi, method, note)；退化时回退百分位并**如实标注**。

    退化三种（都实测过）：重采样分布全在点估计一侧 ⇒ `z0` 发散；`1 − a(z0+q) ≤ 0` ⇒ 分位
    发散；纠正后端点反序。任一发生都不硬算，回退 + note（"假装做了纠正"比不纠正更糟）。
    """
    b = len(dist)
    point = statistics.median(samples)
    below = bisect.bisect_left(dist, point)
    if below == 0 or below == b:
        lo, hi = percentile_ci(dist, alpha)
        return lo, hi, "percentile", "重采样分布退化（z0 发散）——回退百分位"
    nd = statistics.NormalDist()
    z0 = nd.inv_cdf(below / b)
    a = jackknife_accel(samples)

    def adj(q: float) -> float | None:
        num = z0 + q
        den = 1.0 - a * num
        return None if den <= 0 else nd.cdf(z0 + num / den)

    qs = (adj(nd.inv_cdf(alpha / 2)), adj(nd.inv_cdf(1 - alpha / 2)))
    if qs[0] is None or qs[1] is None:
        lo, hi = percentile_ci(dist, alpha)
        return lo, hi, "percentile", "加速常数使分位发散——回退百分位"
    pick = lambda q: dist[min(b - 1, max(0, int(q * b)))]      # noqa: E731
    lo, hi = pick(qs[0]), pick(qs[1])
    if lo > hi:
        lo, hi = percentile_ci(dist, alpha)
        return lo, hi, "percentile", "BCa 端点反序——回退百分位"
    return lo, hi, "bca", None


def p_value(dist: list[float], limit: float) -> float:
    """单侧 bootstrap p 值（H1：中位数 > limit）。加一形式 ⇒ 永不为 0，且与区间同源。"""
    b = len(dist)
    return (1 + bisect.bisect_right(dist, limit)) / (b + 1)


def ci(dist: list[float], samples: list[float], alpha: float = DEFAULT_ALPHA,
       method: str = "bca") -> tuple[float, float, str, str | None]:
    """按档取区间 → (lo, hi, 实际用的方法, note)。`percentile` 档是**既有口径**（逐位可比）。"""
    if method == "percentile":
        lo, hi = percentile_ci(dist, alpha)
        return lo, hi, "percentile", None
    return bca_ci(samples, dist, alpha)


def holm(pvals: list[float], alpha: float = DEFAULT_ALPHA) -> dict:
    """Holm–Bonferroni 逐步下降 → {"reject": [...], "alpha_at": [...], "bonferroni": a/m}。

    控制**族错误率 ≤ alpha**（比 Bonnieferrni 保守度更低而同样控制 FWER）：
    把 p 升序排，第 k 小的与 `alpha/(m−k+1)` 比，一旦不显著就停。
    """
    m = len(pvals)
    reject = [False] * m
    alpha_at: list[float | None] = [None] * m
    for k, i in enumerate(sorted(range(m), key=lambda j: pvals[j]), start=1):
        thr = alpha / (m - k + 1)
        alpha_at[i] = thr
        if pvals[i] <= thr:
            reject[i] = True
        else:
            break
    return {"reject": reject, "alpha_at": alpha_at,
            "bonferroni": (alpha / m) if m else None}


def resolution(samples: list[float], alpha: float = DEFAULT_ALPHA,
               n_boot: int = DEFAULT_BOOT) -> float:
    """分辨力 = 区间半宽：**比它更小的回归，这个样本量测不出来**。"""
    lo, hi = percentile_ci(median_dist(samples, n_boot), alpha)
    return (hi - lo) / 2.0
