"""stat_family.py —— **族级**统计判定（S187 深化，spec/FRONTIER-CI.md 方向⑩）。

单项各自按 α=5% 判，**跑得多就等于更会假红**：m 项各 5% ⇒ 族错误率 ≈ 1−0.95^m
（m=7 约 30%、m=11 约 43%）。多重比较必须校正——这不是洁癖，是"判据自己别造假红"。

做法：逐项 bootstrap p 值（`stat_boot.p_value`）→ **Holm–Bonferroni** 逐步下降
（第 k 小与 α/(m−k+1) 比，一旦不显著即停）⇒ 族错误率 ≤ α。

**样本不足的项不参与族判定**，但如实列进 `weak_items`——把"没测"静默当成"没问题"是最坏的处置。
"""
from __future__ import annotations

from stat_boot import DEFAULT_ALPHA, DEFAULT_BOOT, holm, median_dist, p_value
from stat_judge import MIN_N, judge


def judge_family(items: list[tuple[str, list[float]]], limit: float,
                 alpha: float = DEFAULT_ALPHA, n_boot: int = DEFAULT_BOOT,
                 min_n: int = MIN_N, method: str = "percentile") -> dict:
    """一批（族）样本的判定：逐项三态 + **族级 Holm 校正**。

    族级 `verdict`：有项被 Holm 判显著 ⇒ `fail`（并点名）；有项样本不足 ⇒ `inconclusive`
    （族结论不完整，就别说"没问题"）；否则 `pass`。
    """
    per_item: dict[str, dict] = {}
    usable: list[str] = []
    pvals: list[float] = []
    for name, samples in items:
        got = judge(samples, limit, alpha, n_boot, min_n, method)
        per_item[name] = got
        if got["weak"]:
            continue
        usable.append(name)
        pvals.append(p_value(median_dist(samples, n_boot, f"a{alpha}"), limit))
    h = holm(pvals, alpha)
    regressions = [n for n, r in zip(usable, h["reject"], strict=True) if r]
    weak = [n for n, _ in items if per_item[n]["weak"]]
    return {"verdict": "fail" if regressions else ("inconclusive" if weak else "pass"),
            "regressions": regressions, "weak_items": weak, "family_alpha": alpha,
            "n_tested": len(usable), "bonferroni_alpha": h["bonferroni"],
            "min_resolution": min((i["resolution"] for i in per_item.values()
                                   if i["resolution"] is not None), default=None),
            "p_values": {n: round(p, 4) for n, p in zip(usable, pvals, strict=True)},
            "items": per_item}
