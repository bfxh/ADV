"""tests/test_s186_shard.py —— 分片计划器（S186）：**覆盖等价是红线**，均衡是目标。

背景（实测）：按文件名轮询切分片 ⇒ 3.14 两分片 **256s vs 57s（4.5×）**，墙钟被最慢分片
支配（CI jobs 耗时表见 spec/ROUNDLOG.md S186）。本片改为"按耗时权重 LPT 贪心"——
首要不是更快，而是**不许丢测试**，故覆盖等价是硬断言（丢一个测试就该红）。
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import shard_plan  # noqa: E402


def test_plan_covers_every_test_file_exactly_once():
    files = shard_plan.test_files()
    assert len(files) >= 20, f"tests/ 下测试文件太少？{len(files)}"
    for n in (2, 3, 4):
        buckets = shard_plan.plan(files, shard_plan.load_weights(), n)
        flat = [f for b in buckets for f in b]
        assert sorted(flat) == sorted(files), f"n={n} 覆盖不等价"
        assert len(flat) == len(set(flat)), f"n={n} 有重复项"
        assert all(b for b in buckets), f"n={n} 出现空分片"


def test_verify_rejects_lost_or_duplicated_files():
    """金丝雀：丢掉/重复一个文件必须被判红（否则"均衡"可能以丢覆盖为代价）。"""
    files = shard_plan.test_files()
    lost = [files[1:]]                       # 少一个
    dup = [files, [files[0]]]                # 重复一个
    for buckets in (lost, dup):
        ok, detail = shard_plan.verify(files, buckets)
        assert ok is False and detail, buckets


def test_plan_balances_by_weight():
    """权重悬殊时 LPT 应把重文件摊开：最慢桶不超过平均 1.3×。"""
    files = [f"tests/test_x{i}.py" for i in range(4)]
    weights = {files[0]: 100.0, files[1]: 50.0, files[2]: 10.0, files[3]: 10.0}
    buckets = shard_plan.plan(files, weights, 2)
    loads = [sum(weights[f] for f in b) for b in buckets]
    avg = sum(weights.values()) / 2
    assert max(loads) <= avg * 1.3, loads


def test_missing_weights_still_covered():
    """权重表缺失（新仓/首次）⇒ 回退中位数估，覆盖仍必须等价。"""
    files = shard_plan.test_files()
    buckets = shard_plan.plan(files, {}, 2)
    flat = [f for b in buckets for f in b]
    assert sorted(flat) == sorted(files)


def test_real_plan_verifies_within_threshold():
    """用真实权重表：覆盖等价 + 预估不均衡在阈值内（权重漂移只会变差一点，不会丢覆盖）。"""
    files = shard_plan.test_files()
    buckets = shard_plan.plan(files, shard_plan.load_weights(), 2)
    ok, detail = shard_plan.verify(files, buckets)
    assert ok is True, detail
