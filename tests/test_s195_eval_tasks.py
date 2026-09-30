"""S195：把 S191 的存档规则接进**任务级评测平台**后，给平台本身补齐两件：

1. **测试可达性**（`hub_impact` 的 `impact-static` 判据要求"变更文件有测试可达"，
   而 `bench/tool_evals.py` 是**有顶层副作用的重脚本**（import 即覆盖沙盒 + 建夹具）——
   所以不能放进 `test_bench_smoke` 的导入名单，只能**在用例体内**按路径加载并在
   `finally` 里还原环境，避免污染同进程的其他测试；
2. **两条新任务真的在跑**（不只靠 `--check` 的退出码）：直接调用任务函数断言 ok。

纪律：加载后**必须还原** `UNIFIED_RX_SANDBOX`（tool_evals 顶层会覆盖它，不还原会污染
同进程的 tmp_path 夹具测试）。
"""
import importlib.util
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import registry  # noqa: E402,F401  （确保工具面已注册，任务函数要用）


def _load_tool_evals():
    """按路径加载 bench/tool_evals.py，并**还原沙盒环境**（它顶层会覆盖）。"""
    old = os.environ.get("UNIFIED_RX_SANDBOX")
    try:
        spec = importlib.util.spec_from_file_location("tool_evals", ROOT / "bench" / "tool_evals.py")
        assert spec is not None and spec.loader is not None
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    finally:
        if old is None:
            os.environ.pop("UNIFIED_RX_SANDBOX", None)
        else:
            os.environ["UNIFIED_RX_SANDBOX"] = old


def test_save_tasks_are_registered_on_the_platform():
    mod = _load_tool_evals()
    names = {n for n, _, _ in mod.TASKS}
    assert {"save_rules_fire", "save_rules_quiet"} <= names, names
    assert len(mod.TASKS) >= 15, f"平台任务被削到 {len(mod.TASKS)} 条？"


def test_save_tasks_fire_and_quiet_are_live():
    """真跑两条任务（与 `--check` 同一路径）：违例必报两条 / 正确夹具零命中。"""
    mod = _load_tool_evals()
    rec = mod.Rec()
    ok_fire, detail_fire = mod.t_save_rules_fire(rec)
    ok_quiet, detail_quiet = mod.t_save_rules_quiet(rec)
    assert ok_fire, f"违例夹具必须报出两条存档规则：{detail_fire}"
    assert "save_nonatomic_write" in detail_fire and "save_load_no_version" in detail_fire
    assert ok_quiet, f"原子写 + 版本门在场时不得报：{detail_quiet}"
