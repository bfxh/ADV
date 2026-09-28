"""tests/test_s182_stat.py —— 统计判定（方向⑩）判据：三态正确 + 确定性 + 小样本诚实。

关键：**用真采数据当判据**（2026-09-29 独占机器 7 轮采到的比值）——它们证明"单点比值
会假红"（三次 ≥0.95），也证明"中位数稳定"。
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import stat_judge  # noqa: E402

# ---- 实采数据（独占机器，7 轮；见 spec/ROUNDLOG.md S182） ----
MEASURED = {
    "bug_scan": [0.729, 0.824, 0.654, 0.750, 0.490, 0.728, 0.515],
    "secrets_hunt": [0.515, 0.542, 1.001, 0.478, 0.737, 0.466, 0.528],
    "code_search": [0.694, 0.730, 0.998, 0.508, 0.923, 0.501, 0.833],
    "rust_taint_scan": [0.625, 0.580, 0.546, 0.651, 0.594, 0.694, 0.653],
    "code_semantic": [0.898, 1.034, 0.787, 0.862, 0.815, 0.798, 1.272],
}


def test_separated_distribution_passes():
    v = stat_judge.judge([0.50, 0.52, 0.48, 0.51, 0.49], 0.85)
    assert v["verdict"] == "pass" and v["ci"][1] <= 0.85


def test_clear_regression_fails():
    v = stat_judge.judge([1.10, 1.15, 1.20, 1.12, 1.18], 0.85)
    assert v["verdict"] == "fail" and v["ci"][0] > 0.85


def test_measured_bug_scan_is_pass_over_measured_secrets_is_pass():
    """实采：bug_scan / secrets_hunt 的中位数远低于 0.85 ⇒ pass（单点曾抽到 1.001，
    但统计口径不把它当真回归——这正是本模块存在的理由）。"""
    for label in ("bug_scan", "secrets_hunt", "rust_taint_scan"):
        v = stat_judge.judge(MEASURED[label], 0.85)
        assert v["verdict"] == "pass", (label, v)
    assert stat_judge.judge(MEASURED["secrets_hunt"], 0.85)["max"] > 1.0


def test_measured_semantic_is_not_fail_in_noise_band():
    """实采：code_semantic 在限 0.95 下**不得**被判 fail（中位数 0.862；
    区间可能跨阈 ⇒ 允许 inconclusive；判 fail 就是把噪声当结论）。"""
    v = stat_judge.judge(MEASURED["code_semantic"], 0.95)
    assert v["verdict"] in ("pass", "inconclusive"), v
    assert abs(v["median"] - 0.862) < 0.01


def test_deterministic_same_input_same_output():
    a = stat_judge.judge(MEASURED["code_semantic"], 0.95)
    b = stat_judge.judge(MEASURED["code_semantic"], 0.95)
    assert a == b, "同输入必须同输出（Bootstrap 播种）"


def test_small_sample_is_honest():
    v = stat_judge.judge([0.9, 0.8], 0.85)
    assert v["verdict"] == "inconclusive" and v["weak"] is True and v["reason"]
    v0 = stat_judge.judge([], 0.85)
    assert v0["verdict"] == "inconclusive" and v0["n"] == 0
