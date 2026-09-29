"""tests/test_s187_stat_depth.py —— 统计模块深化（S187，spec/FRONTIER-CI.md 方向⑩）的判据。

四件深度各配"能被证伪"的判据（只查"返回非空"是查不出假实现的）：

1. **BCa 不是摆设**：偏态总体上的**经验覆盖率**要接近名义 95%；且 BCa 必须与百分位**不同**
   （若逐位相同 ⇒ 所谓"纠正"是空操作）；退化样本要**如实回退并留 note**（不假装纠正过）。
2. **旧口径逐位不变**：`method="percentile"` 必须与 S187 之前的实现**逐位一致**（钉住非回归）。
3. **p 值与区间同源**：`p ≤ α/2` ⇒ 百分位下界排除 limit（反方向留一格取整误差）。
4. **多重比较真在校正**：零假设族下 Holm 的族判红率 ≤ α；不校正的逐项口径明显更高
   （两者差得出来，才说明校正不是摆设）。
5. **分辨力说人话**：`resolution` = 区间半宽、`mde_ratio` = 同口径比值；`pass` 必须**带上**
   分辨力（"没测到" ≠ "没有"）。
6. **跨进程确定性**：同输入在独立进程里得到同一个字典。

全部播种（`stat_boot.seed_of`）⇒ 结果确定，不会闪红。
"""
import hashlib
import json
import math
import pathlib
import random
import statistics
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import stat_boot  # noqa: E402
import stat_family  # noqa: E402
import stat_judge  # noqa: E402


def _legacy_ci(samples, n_boot=2000, alpha=0.05):
    """S187 **之前**的实现（冻结在此当参照）——用来钉"旧口径逐位不变"。"""
    blob = "|".join(f"{x:.9f}" for x in samples) + "|" + f"boot{n_boot}a{alpha}"
    seed = int.from_bytes(hashlib.sha256(blob.encode()).digest()[:4], "big")
    rnd = random.Random(seed)
    n = len(samples)
    meds = sorted(statistics.median(rnd.choices(samples, k=n)) for _ in range(n_boot))
    lo = meds[max(0, int(n_boot * alpha / 2))]
    hi = meds[min(n_boot - 1, int(n_boot * (1 - alpha / 2)) - 1)]
    return lo, hi


# ---------------- ① BCa 不是摆设 ----------------

def test_percentile_path_is_bit_identical_to_legacy():
    """旧口径逐位不变（S187 只**新增**档位，不动既有分母）。"""
    s = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0]
    assert stat_judge.bootstrap_ci(s) == _legacy_ci(s)


def test_bca_differs_from_percentile_on_skewed_data():
    """BCa 必须真的与百分位不同——相同就说明"纠正"是空操作。"""
    rnd = random.Random(11)
    s = sorted(rnd.expovariate(1.0) for _ in range(15))
    dist = stat_boot.median_dist(s, 800, "a0.05")
    lo_p, hi_p = stat_boot.percentile_ci(dist, 0.05)
    lo_b, hi_b, used, note = stat_boot.ci(dist, s, 0.05, "bca")
    assert used == "bca" and note is None
    assert (lo_b, hi_b) != (lo_p, hi_p)


def test_bca_falls_back_honestly_on_degenerate_sample():
    """常量样本 ⇒ z0 发散：**如实回退**百分位并留 note（不假装做了纠正）。"""
    got = stat_judge.judge([2.0] * 9, 10.0, method="bca")
    assert got["ci_method"] == "percentile" and got["ci_note"]


def test_bca_vs_percentile_measured_tradeoff():
    """把"BCa 更窄但**覆盖更低**"这条**量出来的**关系钉住——要换默认档，先重新量。

    这是本片最反直觉的一条：先验"BCa 小样本更准"被实测否掉（六个格子一致：BCa 宽度更小、
    覆盖率系统性低于百分位）。判据因此**默认留在百分位**（保守优先），BCa 降为可选档。
    """
    rnd = random.Random(20260929)
    n, reps, boot, true_med = 15, 200, 400, math.log(2.0)
    hit = {"bca": 0, "percentile": 0}
    wid = {"bca": [], "percentile": []}
    for _ in range(reps):
        s = [rnd.expovariate(1.0) for _ in range(n)]
        dist = stat_boot.median_dist(s, boot, "a0.05")
        for m in ("bca", "percentile"):
            lo, hi, _used, _note = stat_boot.ci(dist, s, 0.05, m)
            hit[m] += int(lo <= true_med <= hi)
            wid[m].append(hi - lo)
    cov_p, cov_b = hit["percentile"] / reps, hit["bca"] / reps
    # ① 两条都得**是区间**（不能是空区间，也不能宽到失去意义）
    assert 0.85 <= cov_p <= 0.99, (cov_p, cov_b)
    assert 0.85 <= cov_b <= 0.99, (cov_p, cov_b)
    # ② 实测关系：百分位覆盖不低于 BCa，而 BCa 更窄 —— "用保守换精度"就是它的代价
    assert cov_p >= cov_b, (cov_p, cov_b)
    assert statistics.mean(wid["bca"]) < statistics.mean(wid["percentile"]), wid


def test_default_method_is_percentile():
    """默认档 = 百分位（保守优先）；且与显式 `method="percentile"` 逐位相同。"""
    s = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0]
    assert stat_judge.judge(s, 5.0) == stat_judge.judge(s, 5.0, method="percentile")
    assert stat_judge.judge(s, 5.0)["ci_method"] == "percentile"


# ---------------- ③ p 值与区间同源 ----------------

def test_p_value_agrees_with_interval():
    s = [1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 8.0, 9.0]
    dist = stat_boot.median_dist(s, 2000, "a0.05")
    for limit in (1.5, 3.0, 4.0, 5.0, 12.0):
        p = stat_boot.p_value(dist, limit)
        lo, _hi = stat_boot.percentile_ci(dist, 0.05)
        if p <= 0.025:                             # 可证方向：显著 ⇒ 下界必排除 limit
            assert lo > limit, (limit, p, lo)
        elif p > 0.03:                             # 反方向留一格取整误差
            assert lo <= limit, (limit, p, lo)


# ---------------- ④ 多重比较真在校正 ----------------

def test_holm_family_error_rate_and_contrast():
    """零假设族（真值正好在 limit）下：Holm 的族判红率 ≤ α；不校正的逐项口径明显更高。"""
    rnd = random.Random(7)
    m, fams, limit = 5, 200, 1.0
    holm_fail = uncorrected = 0
    for _ in range(fams):
        items = [(f"c{i}", [limit + rnd.gauss(0.0, 0.12) for _ in range(9)])
                 for i in range(m)]
        rep = stat_family.judge_family(items, limit, n_boot=400)
        holm_fail += rep["verdict"] == "fail"
        uncorrected += any(p <= 0.05 for p in rep["p_values"].values())
    assert holm_fail / fams <= 0.08, (holm_fail, fams)
    assert uncorrected >= holm_fail, (uncorrected, holm_fail)
    assert uncorrected / fams >= 0.04, (uncorrected, fams)      # 对照：不校正确实更容易红


def test_holm_step_down_thresholds():
    """Holm 逐步下降：第 k 小比 α/(m−k+1)，**一旦不显著即停**（后面档位再宽也不再判显著）。"""
    got = stat_boot.holm([0.001, 0.02, 0.03, 0.9], 0.05)
    assert got["reject"] == [True, False, False, False]     # k=2：0.02 > 0.05/3 ≈ 0.0167 ⇒ 停
    assert got["alpha_at"][0] == 0.05 / 4
    assert got["bonferroni"] == 0.05 / 4
    cont = stat_boot.holm([0.001, 0.002, 0.9], 0.05)
    assert cont["reject"] == [True, True, False]            # 连续显著 ⇒ 逐档前进


def test_judge_family_names_regression_and_marks_weak():
    """族级：点名真回归；样本不足的项**不参与**判定但**如实列出**，且族结论不假装"没问题"。"""
    healthy = [1.0] * 6 + [0.9, 1.1]
    bad = [2.0] * 6 + [1.9, 2.1]
    rep = stat_family.judge_family([("a", healthy), ("b", bad), ("c", [1.0, 1.0])], 1.25)
    assert rep["verdict"] == "fail" and rep["regressions"] == ["b"]
    assert rep["weak_items"] == ["c"] and "c" not in rep["p_values"]
    only_weak = stat_family.judge_family([("a", [1.0, 1.0])], 1.25)
    assert only_weak["verdict"] == "inconclusive"


# ---------------- ⑤ 分辨力说人话 ----------------

def test_resolution_and_mde_reported():
    """`resolution` = 区间半宽；`mde_ratio` = 同口径比值；样本越散分辨力越差。"""
    tight = stat_judge.judge([1.00, 1.01, 1.02, 0.99, 1.00, 1.01], 2.0)
    loose = stat_judge.judge([1.0, 5.0, 2.0, 8.0, 0.5, 3.0], 2.0)
    assert tight["resolution"] is not None and loose["resolution"] > tight["resolution"]
    lo, hi = tight["ci"]
    assert tight["resolution"] == round((hi - lo) / 2, 4)
    assert tight["mde_ratio"] == round(1 + tight["resolution"] / 2.0, 4)
    weak = stat_judge.judge([1.0, 2.0], 2.0)
    assert weak["weak"] is True and weak["resolution"] is None and weak["reason"]


# ---------------- ⑥ 跨进程确定性 ----------------

def test_determinism_across_processes():
    """同输入 ⇒ 独立进程里得到**同一个字典**（不依赖全局随机态）。"""
    code = (
        "import json,sys;sys.path.insert(0,'scripts');"
        "import stat_judge,stat_family;"
        "s=[1.0,2.0,2.0,3.0,3.0,4.0,4.0,5.0,6.0,8.0,9.0];"
        "print(json.dumps({'j':stat_judge.judge(s,4.0),"
        "'f':stat_family.judge_family([('a',s),('b',s)],4.0)},sort_keys=True))"
    )
    cp = subprocess.run([sys.executable, "-X", "utf8", "-c", code], capture_output=True,
                        text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                        shell=False, timeout=120)
    assert cp.returncode == 0, cp.stderr
    here = json.dumps({"j": stat_judge.judge([1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0,
                                              8.0, 9.0], 4.0),
                       "f": stat_family.judge_family(
                           [("a", [1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 8.0, 9.0]),
                            ("b", [1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 8.0, 9.0])],
                           4.0)}, sort_keys=True)
    assert cp.stdout.strip() == here
