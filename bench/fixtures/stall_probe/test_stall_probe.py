"""S208 探针：故意睡过 stall 上限，给 tests/test_s208_stall_watchdog.py 用 subprocess 真跑。

放在 `bench/fixtures/` 下而不是 `tests/`：`shard_plan.py` 只 glob `tests/test_*.py`，
所以这个文件**不进分片计划、不进常规套件**——只有金丝雀显式把它交给 pytest。
睡时长走 env（默认 6 秒），金丝雀把 stall 上限设成 1 秒来触发转储。
"""
import os
import time


def test_stall_probe():
    time.sleep(float(os.environ.get("UNIFIED_RX_STALL_PROBE_S", "6")))
