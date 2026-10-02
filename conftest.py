"""conftest.py —— pytest 全局配置：tmp 基目录在 %TEMP%\\unified-rx-pytest，
并把该前缀加入沙盒放行（保持 fail-closed 语义：仅此显式白名单 + 项目根）。

不再把 _pytest_tmp 放仓库根（UPGRADE-A2）：夹具残留会污染 bug_scan 自扫与 git。
"""
import faulthandler
import os
import tempfile

# pytest 默认跑在 fail-closed 沙盒内：未设置时 = 项目根 + 专用 tmp 前缀
# （runner 上真实工作区不存在，用 checkout 根替代——等价且可移植）
_TMP_BASE = os.path.join(tempfile.gettempdir(), "unified-rx-pytest")
os.makedirs(_TMP_BASE, exist_ok=True)
_WORKSPACE = os.path.dirname(os.path.abspath(__file__))
os.environ.setdefault("UNIFIED_RX_SANDBOX", os.pathsep.join([
    _WORKSPACE,           # 真实工作区（本机=D:\开发，CI=checkout 根）
    _TMP_BASE,            # pytest 夹具专用前缀（进程内显式授权）
]))
os.environ["UNIFIED_RX_SANDBOX"] = os.environ["UNIFIED_RX_SANDBOX"].replace(os.pathsep, ";")

# S122：套件里有大量"故意重复同一工具+参数"的测试（缓存命中/压力/稳定性），
# 熔断器默认旁路；熔断自身行为由 tests/test_s122_breaker.py 显式开启验证。
os.environ["UNIFIED_RX_BREAKER"] = "off"

# S140：全局护栏在套件里默认高阈值/关闭（与 UNIFIED_RX_BREAKER=off 同理），
# 全局 QPM/日告警行为由 tests/test_s122_breaker.py 显式开启验证。
os.environ.setdefault("UNIFIED_RX_GLOBAL_QPM", "1000000")
os.environ.setdefault("UNIFIED_RX_DAILY_ALERT", "0")
# S141：会话烧量哨兵默认关（专项测试显式开——否则跑套件期间会读真实 rollout 目录）
os.environ.setdefault("UNIFIED_RX_BURN_MB", "0")

tempfile.tempdir = _TMP_BASE


# 并发/分片隔离（2026-10-01，实测判据：4 个 pytest 进程共享同一 basetemp 时，
# 同编号 tmp 目录被互相 rmtree/重建 ⇒ ~120-170 个 setup errors 且逐轮波动）：
# basetemp 按 PID 派生——每个 pytest 进程独享自己的编号目录空间；
# 共享根 _TMP_BASE 仍在（沙盒放行与硬编码引用它的测试不受影响）。
def pytest_configure(config):
    config.option.basetemp = os.path.join(_TMP_BASE, f"proc-{os.getpid()}")


import pytest

# S140：测试打点落点——放 basetemp 下的独立目录，而不是 tmp_path：
# 一批测试直接把 tmp_path 当语料根做全量对账，塞进 stats.jsonl 会多出 1 个条目。
# 并发：随 basetemp 一起按 PID 分根，四个进程不再互写同一 stats.jsonl。
_STATS_TMP = os.path.join(_TMP_BASE, f"proc-{os.getpid()}", "_stats")
os.makedirs(_STATS_TMP, exist_ok=True)

# S151：常驻服务在套件里默认**关闭**——测试走 spawn 路径（与历史行为一致），
# 服务本身由 tests/test_s151_svc.py 显式开启并逐字节对账。
os.environ.setdefault("UNIFIED_RX_SVC", "off")

# S146：握手留痕同样隔离——tests/test_v2 直接 _handle(initialize) 会把空 params
# 裸写进真实 ~/.ADV/clients.jsonl（首轮留痕 4 条 null 全来自测试，实锤
# 归因困难）。审计账本只许装真实宿主握手，测试一律写 tmp。
_CLIENTS_TMP = os.path.join(_STATS_TMP, "clients.jsonl")
os.environ["UNIFIED_RX_CLIENTS_LOG"] = _CLIENTS_TMP


@pytest.fixture(autouse=True)
def _isolate_stats(monkeypatch):
    """S140：测试打点一律落 tmp 隔离区——套件/bench 的 registry.call 不再写进
    真实 ~/.ADV/stats.jsonl（9/8 统计污染的教训之一）。
    S141：日计数落点同样隔离（熔断开启的测试不碰真实 daily_state.jsonl）。"""
    import registry
    target = os.path.join(_STATS_TMP, "stats.jsonl")
    monkeypatch.setattr(registry, "_stats_path", lambda: target)
    try:
        from tools import breaker as _breaker
        monkeypatch.setattr(_breaker, "_daily_path",
                            lambda: os.path.join(_STATS_TMP, "daily_state.jsonl"))
    except Exception:                                              # noqa: BLE001
        pass


# 沙盒 git 消毒（2026-10-01，P0）：10+ 个测试文件在临时仓跑 git（-c user.name=t…）。
# 被测产品代码会改写本进程 os.environ 的 GIT_*（GIT_DIR/GIT_INDEX_FILE/…），而
# GIT_DIR 的优先级高于 git 的 -C ⇒ 下一支测试的沙盒 git 可能落进**宿主真仓**
# （实测判据：worktree 里跑全量套件后，宿主仓出现 t@t 的 junk commit 与 v9.9.0
# 假 tag）。本夹具：每支测试前剥掉全部 GIT_*；测试后清掉测试期间新增的
# （污染不出测试边界）。test_s180 的 GIT_DIR 注入回归自行 setenv，不受影响。
_GIT_ENV_KEYS = (
    "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES", "GIT_CONFIG", "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM", "GIT_CONFIG_PARAMETERS", "GIT_PREFIX",
    "GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL", "GIT_INDEX_VERSION",
)


@pytest.fixture(autouse=True)
def _sanitize_git_env(monkeypatch):
    before = {k for k in os.environ if k.startswith("GIT_")}
    for k in before | set(_GIT_ENV_KEYS):
        monkeypatch.delenv(k, raising=False)
    yield
    for k in list(os.environ):
        if k.startswith("GIT_") and k not in before:
            del os.environ[k]


# S208：测试卡死护栏——纯 stdlib faulthandler，不引 pytest-timeout（零依赖红线）。
# 没有它时，一条挂住的测试的默认结局是"整片静默耗到 local_gate 的 3600 秒上限"，现场全丢。
# 两个实测踩出来的设计约束：
#  1) 转储**不能打 stderr**：pytest 按测试捕获，测试最终通过时那段栈会被直接丢弃——而
#     "很慢但还是过了"恰恰是最需要看它的时候（金丝雀第一版就是这么判红的）。所以落文件。
#  2) **只转储不杀进程**：慢机器/负载高时的慢不等于回归（与计时门必须独占机器同一纪律）。
# env：UNIFIED_RX_TEST_STALL_S（默认 180 秒，0=关）；UNIFIED_RX_TEST_STALL_LOG（转储文件）。
_STALL_S = float(os.environ.get("UNIFIED_RX_TEST_STALL_S", "180") or 0)
def _bounded_stall_log():
    """转储文件只许落在临时目录下——env 能把路径指到任意位置，那就是一个"外部输入决定
    写路径"的 sink（数据流门实测会点它）。越界就**关护栏并说出来**，不悄悄换个地方写。"""
    raw = (os.environ.get("UNIFIED_RX_TEST_STALL_LOG") or "").strip()
    if not raw:
        return os.path.join(_TMP_BASE, "stall-dump.log"), ""
    real = os.path.realpath(raw)
    if os.path.commonpath([real, os.path.realpath(tempfile.gettempdir())]) !=             os.path.realpath(tempfile.gettempdir()):
        return "", f"env 指定的转储路径不在临时目录内，已拒绝并关闭护栏：{raw}"
    return real, ""


_LOG_OR_WHY = _bounded_stall_log()
_STALL_LOG = _LOG_OR_WHY[0]
_STALL_DISABLED_WHY = _LOG_OR_WHY[1]
_STALL_FH = None

if _STALL_S > 0 and _STALL_LOG:
    try:
        if os.path.isfile(_STALL_LOG):
            os.remove(_STALL_LOG)          # 不留上一轮的旧证据
        # 长驻句柄：转储目标必须活到 session 结束，由 pytest_sessionfinish 关闭，
        # 所以这里不能用 with（否则 hook 注册时句柄已关）。
        _STALL_FH = open(_STALL_LOG, "a", encoding="utf-8")  # noqa: SIM115
    except OSError as e:
        _STALL_DISABLED_WHY = f"转储文件打不开（{_STALL_LOG}）：{e}"


def pytest_report_header(config):
    if _STALL_FH is not None:
        return f"stall-watchdog: 单条测试超过 {_STALL_S:.0f}s 就把全部线程栈追加到 {_STALL_LOG}"
    if _STALL_S > 0:
        return f"stall-watchdog: 未启用——{_STALL_DISABLED_WHY}"
    return "stall-watchdog: 已关闭（UNIFIED_RX_TEST_STALL_S=0）"


if _STALL_FH is not None:
    def pytest_runtest_setup(item):
        faulthandler.dump_traceback_later(_STALL_S, repeat=True, file=_STALL_FH)

    def pytest_runtest_teardown(item, nextitem):
        faulthandler.cancel_dump_traceback_later()

    def pytest_sessionfinish(session, exitstatus):
        faulthandler.cancel_dump_traceback_later()
        _STALL_FH.close()
