"""tests/test_s187_truncation.py —— 账本**截断**判据（S187 续，方向⑥ 第二片）。

命题：**"账本被写坏"与"那行从没写过"必须可区分**。

修前实录：末行截断时 `verify_chain` 报 **`ok=True`**——截断行被解析器静默丢掉，剩下的前缀
是自洽的，于是坏账本看起来像好账本（假绿），而那条运行永远停在 `start`（看着像还在跑）。
中间行整条丢失反而一直能被 `prev_hash` 抓到 ⇒ **缺口只在末行**。

为什么另立文件（而不是加进 `test_s183_resilience.py`）：**god 门棘轮对测试文件同样生效**
（那个文件变胖会红，而它的最长函数只有 12 行、换不出空间）——"加测试"也走新文件这条路。

夹具刻意不跑管线（只要一本自洽的账）：`hub_core.append_row` 直接造，省掉临时管线目录，
也让"注入"这一步是测试里唯一被观察的动作。
"""
import pytest

import hub_core
import hub_ledger


@pytest.fixture()
def env(tmp_path, monkeypatch):
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(tmp_path / "hub"))
    return tmp_path / "hub"


def _seed() -> None:
    """两行自洽的账（start + final），链校验必绿。"""
    hub_core.append_row({"id": "r-1", "phase": "start"})
    hub_core.append_row({"id": "r-1", "phase": "final", "verdict": "green"})


def test_truncated_tail_is_not_a_green_chain(env):
    """末行被写坏（半条 JSON）⇒ **不许报"链 OK"**，且坏行要被**数出来**。"""
    _seed()
    led = hub_core.runs_path()
    whole = led.read_text(encoding="utf-8")
    assert hub_core.verify_chain()["ok"] is True
    led.write_text(whole.rstrip("\n")[:-12], encoding="utf-8")
    ch = hub_core.verify_chain()
    assert ch["ok"] is False and "截断/损坏" in str(ch["reason"]), ch
    assert ch["bad_lines"] == [2]
    led.write_text(whole, encoding="utf-8")
    assert hub_core.verify_chain()["ok"] is True          # 对照：仪器不是恒红


def test_truncated_tail_without_newline_is_red(env):
    """末行**解析得了但没写完**（断在合法 JSON 边界）——只有"换行契约"抓得到。"""
    _seed()
    led = hub_core.runs_path()
    led.write_text(led.read_text(encoding="utf-8").rstrip("\n"), encoding="utf-8")
    ch = hub_core.verify_chain()
    assert ch["ok"] is False and "换行" in str(ch["reason"]), ch


def test_missing_middle_line_is_red(env):
    """中间行整条丢失 ⇒ prev_hash 断链判红（这条一直能抓，钉住不许退化）。"""
    _seed()
    led = hub_core.runs_path()
    lines = led.read_text(encoding="utf-8").rstrip("\n").split("\n")
    led.write_text(lines[1] + "\n", encoding="utf-8")
    ch = hub_core.verify_chain()
    assert ch["ok"] is False and "prev_hash" in str(ch["reason"]), ch


def test_bad_lines_are_counted_not_silently_dropped():
    """`parse_rows` 必须**数出**坏行（跳过 ≠ 不存在）——纯函数，不碰账本。"""
    good = '{"a": 1}'
    rows, bad = hub_ledger.parse_rows(good + "\n" + "{oops\n" + good + "\n")
    assert len(rows) == 2 and bad == [2]


def test_read_rows_still_tolerates_bad_lines(env):
    """读路径的**容错语义不变**（S183 纪律）：坏行仍被跳过、不抛——只是链判据会报出来。"""
    _seed()
    led = hub_core.runs_path()
    led.write_text(led.read_text(encoding="utf-8").rstrip("\n")[:-12], encoding="utf-8")
    assert len(hub_core.read_rows()) == 1                  # 不做异常，只降级
    assert hub_core.verify_chain()["ok"] is False          # 但**不许**报绿


def test_hub_runs_reports_broken_chain(env):
    """平台面：账本坏了 ⇒ `hub_runs` 的 chain 如实报（不带上坏账本报绿）。"""
    _seed()
    led = hub_core.runs_path()
    led.write_text(led.read_text(encoding="utf-8").rstrip("\n")[:-8], encoding="utf-8")
    import tools.hub as hub_tools
    assert hub_tools.hub_runs(5)["chain"]["ok"] is False
