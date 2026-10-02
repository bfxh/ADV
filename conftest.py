"""conftest.py —— pytest 全局配置：tmp 基目录在 %TEMP%\\unified-rx-pytest，
并把该前缀加入沙盒放行（保持 fail-closed 语义：仅此显式白名单 + 项目根）。

不再把 _pytest_tmp 放仓库根（UPGRADE-A2）：夹具残留会污染 bug_scan 自扫与 git。
"""
import contextlib
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
# 没有它时，一条挂住的测试会静默耗到 local_gate 的 3600 秒上限，现场全丢。
# **只转储不杀进程**：负载高时的慢不等于回归（与计时门必须独占机器同一条纪律）。
#
# 转储目标被三次实锤推到现在的形状（改回去就会重现对应故障）：
#  1) `sys.stderr`：pytest 按测试捕获，测试**最终通过**时栈被丢弃——而"很慢但还是过了"
#     正是最需要它的场景（自家金丝雀判红）。`sys.__stderr__` 也不行：pytest 的 fd 捕获
#     把 fd 2 整个 dup 走了，原始对象写的还是那个被换掉的 fd 号。
#  2) 自持的 Python 文本文件对象：CI 的 Python 3.11 矩阵上内层进程直接 `access
#     violation`（rc=0xC0000005）崩在 `_pytest/terminal.py` 报告阶段——看门狗线程与主线程
#     并发进入同一个缓冲写对象 + 句柄生命周期在我们手里。
#  3) 现方案：给 faulthandler 一个**裸 fd**（`os.open` 的整数返回，C 层直写，不经任何
#     Python 写对象），落到临时目录下的日志文件；关闭前先 cancel。既不被捕获，也没有共享写者。
# env：UNIFIED_RX_TEST_STALL_S（默认 180 秒，0=关）；UNIFIED_RX_TEST_STALL_LOG（转储路径，
# 只许临时目录内——"env 决定写路径"会被数据流门判 sink，越界即拒并关护栏）。
_STALL_S = float(os.environ.get("UNIFIED_RX_TEST_STALL_S", "180") or 0)


def _bounded_stall_log():
    """转储路径必须在临时目录内；env 指到外面就关护栏并说明原因（不换个地方偷偷写）。"""
    raw = (os.environ.get("UNIFIED_RX_TEST_STALL_LOG") or "").strip()
    if not raw:
        return os.path.join(tempfile.gettempdir(), "unified-rx-stall-dump.log"), ""
    tmp_root = os.path.realpath(tempfile.gettempdir())
    real = os.path.realpath(raw)
    if os.path.commonpath([real, tmp_root]) != tmp_root:
        return "", f"env 指定的转储路径不在临时目录内，已拒绝并关闭护栏：{raw}"
    return real, ""


_STALL_LOG, _STALL_REJECT_WHY = _bounded_stall_log()
_STALL_FD = -1

if _STALL_S > 0 and _STALL_LOG:
    try:
        _STALL_FD = os.open(_STALL_LOG, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    except OSError as e:                       # 开不出来就关护栏，但必须说得出原因
        _STALL_FD = -1
        _STALL_REJECT_WHY = f"转储文件打不开（{_STALL_LOG}）：{e}"

_STALL_ON = _STALL_S > 0 and _STALL_FD >= 0


def pytest_report_header(config):
    if _STALL_ON:
        return f"stall-watchdog: 单条测试超过 {_STALL_S:.0f}s 就把全部线程栈追加到 {_STALL_LOG}"
    if _STALL_S > 0:
        return f"stall-watchdog: 未启用——{_STALL_REJECT_WHY}"
    return "stall-watchdog: 已关闭（UNIFIED_RX_TEST_STALL_S=0）"


if _STALL_ON:
    def pytest_runtest_setup(item):
        faulthandler.dump_traceback_later(_STALL_S, repeat=True, file=_STALL_FD)

    def pytest_runtest_teardown(item, nextitem):
        faulthandler.cancel_dump_traceback_later()

    def pytest_sessionfinish(session, exitstatus):
        faulthandler.cancel_dump_traceback_later()   # 先停表再关 fd，反过来会撞上在途写
        with contextlib.suppress(OSError):
            os.close(_STALL_FD)
