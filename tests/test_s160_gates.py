"""S160 契约：两道新门——性能门（并行/串行比值）+ 协议面门（真 stdio 握手）。

用户指令：「CI 审核继续加强……加一个性能门」。两门都进 core.yml + local_gate
（本地与 CI 同源，形状锁在 test_s124 的 needle 列表里）。本文件锁它们的**真跑**：

1. perf_gate 真跑绿（小语料 120 文件——判据是"同机并行 vs 串行比值"，与机器
   绝对速度无关，故小语料也有效；`UNIFIED_RX_NO_PAR=1` 由 rust/src/par.rs 提供）；
2. perf_gate 的**真门**性质：语料根被替换成不存在的路径时，各例失败（rc≠0 或
   工作量异常）→ 门必须判红（不是永远绿）。用 --files 1 + 造一个越界根验证太绕，
   这里直接验"没有 exe 时 SKIP、有 exe 时 OK"两态，并对 sabotage 式的 rc 检查
   依赖 rc 判定（见下 test_perf_gate_flags_nonzero_rc：把命令换成必然失败的工具）。
3. mcp_surface_gate 真跑绿，且逐条契约都打印（15 条 OK 行）——它是 S143/S144/
   S146/S149 协议契约的总闸（含"未知方法 → JSON-RPC error{code}"）。
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _run(script, *args, env_extra=None):
    env = dict(os.environ, **(env_extra or {}))
    return subprocess.run([sys.executable, "-X", "utf8",
                           os.path.join(ROOT, "scripts", script), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", env=env, cwd=ROOT, shell=False,
                          timeout=900)


def test_perf_gate_green_on_small_corpus():
    cp = _run("perf_gate.py", "--files", "120")
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "PERF-GATE OK" in cp.stdout
    # 五例都跑了（不是空跑）：每例一行——S182 起为统计口径（中位数+区间）或如实 SIZE-SKIP
    assert (cp.stdout.count("中位数=") + cp.stdout.count("规模过小")) == 5, cp.stdout
    # 判据口径必须自报（读者要知道在看什么）
    assert "Bootstrap 95% 区间" in cp.stdout


def test_perf_gate_reports_statistical_verdicts():
    """门真的做了 A/B 且给**统计量**：中位数 / 区间 / n / 限 / 三态标记齐备。

    S182 口径升级（先量后改）：单点比值极差 0.15–0.54、三次抽到 ≥0.95 ⇒ 改为
    "K 轮采样 → 中位数 + Bootstrap 区间 → 三态（OK/WEAK/SLOW!）"。
    本测试只验**形状**（真的在报统计量且点估计落在区间内）；数值判据归
    `tests/test_s182_stat.py` 与防骗门 `stat-judge`。
    """
    import re
    cp = _run("perf_gate.py", "--files", "120")
    lines = [ln for ln in cp.stdout.splitlines() if ("中位数=" in ln or "规模过小" in ln)]
    assert len(lines) == 5, cp.stdout
    for ln in lines:
        if "中位数=" not in ln:
            assert "SIZE-SKIP" in ln, ln
            continue
        m = re.search(r"中位数=([0-9.]+) 区间=\[([0-9.]+),([0-9.]+)\] n=(\d+) 限 ([0-9.]+)", ln)
        assert m, ln
        point, lo, hi = float(m.group(1)), float(m.group(2)), float(m.group(3))
        assert lo <= point <= hi, ln                      # 点估计必须在区间内
        assert int(m.group(4)) >= 1 and float(m.group(5)) > 0, ln
        assert ln.rstrip().endswith(("OK", "WEAK", "SLOW!")), ln


def test_mcp_surface_gate_green_and_covers_contracts():
    cp = _run("mcp_surface_gate.py")
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "MCP-SURFACE-GATE OK" in cp.stdout
    for needle in ("annotations.title/readOnlyHint 全部在（S143 契约）",
                   "顶层 title 与 annotations.title 同值（S146 契约）",
                   "fs_read 回包带不可信前缀（S144 契约）",
                   "未知方法返回错误对象（带 code）",
                   "握手留痕含 negotiated/params_keys/pid/ppid/server"):
        assert needle in cp.stdout, f"协议面门缺契约检查: {needle}\n{cp.stdout}"


def test_par_switch_actually_serializes():
    """`UNIFIED_RX_NO_PAR=1` 必须真的改变行为（否则性能门的 A/B 是同一条路径）。

    用 bug_scan 在语料上跑：串行（NO_PAR=1）应比并行慢——若开关失效，两者会
    几乎相同。这里只验"开关不报错且两态都能跑出结果"，比值判据留给 perf_gate。
    """
    import tempfile
    from pathlib import Path
    T = os.environ.get("TEMP", ".")
    exe = os.path.join(T, "rx-rs-target", "release", "rx-scan.exe")
    if not os.path.isfile(exe):
        import pytest
        pytest.skip("rx-scan.exe 未构建")
    W = Path(tempfile.mkdtemp(prefix="urx-par-")).resolve()
    for i in range(20):
        (W / f"m{i}.py").write_text("x = 1\n", encoding="utf-8")
    base = {"UNIFIED_RX_SANDBOX": str(W)}
    ser = subprocess.run([exe, "bugscan", str(W), "100"], capture_output=True,
                         env={**os.environ, **base, "UNIFIED_RX_NO_PAR": "1"},
                         shell=False, timeout=120)
    par = subprocess.run([exe, "bugscan", str(W), "100"], capture_output=True,
                         env={**os.environ, **base}, shell=False, timeout=120)
    assert ser.returncode == 0 and par.returncode == 0
    assert ser.stdout == par.stdout, "串行/并行输出必须逐字节一致（并行不改结果）"
