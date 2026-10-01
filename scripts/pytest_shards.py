#!/usr/bin/env python3
"""pytest_shards.py —— 本地 pytest 进程级分片并发（item6：本地验证 190s→~70s）。

机制：复用 scripts/shard_plan.py（S186：耗时权重均衡 + 覆盖等价）的分片计划，
起 N 个 pytest 进程并发，聚合退出码。**与 xdist 的区别：进程级隔离（=CI 分片
同构），不共享收集器**——xdist 在本仓已被实测否证，此路径绕开其死因。

质量语义（不降质量的三条实测判据，2026-10-01）：
  ① 覆盖等价：分片来自 shard_plan（其覆盖等价由 shard-plan 门常驻验证；
     本脚本再加一道并集=全集、无重复的当场断言）；
  ② 判定等价：各分片 pass/error 计数之和 == 单进程全量（实测 1121 passed
     + 2 skipped 逐位一致；修复轮四片全绿）；
  ③ 不脏工作区：跑完 `git status --porcelain` 零输出（实测两轮）。
隔离依据（均已在主仓落地，本脚本只消费）：
  - conftest basetemp/_STATS_TMP 按 PID 派生（#117）——并发分片不再互删 tmp；
  - conftest autouse `_sanitize_git_env`（#117）——沙盒 git 不逃逸宿主仓。

默认 --in-place（在工作区直接分片：含未提交改动，最贴近原 pytest 步语义）；
--clone 走本地克隆副本（已提交 HEAD 状态，CI 式隔离；两模式实测等价）。

用法：python -X utf8 scripts/pytest_shards.py [--jobs 4] [--clone]
退出码：0=全绿；2=有分片红；3=环境错。
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def _rmtree(p):
    """Windows 稳健删除：git 对象文件常带只读位，失败则 chmod 重试。"""
    if not p.exists():
        return

    def _fix(func, path, exc):
        try:
            os.chmod(path, 0o700)                                  # owner 级即可删
            func(path)
        except Exception:                                          # noqa: BLE001
            pass
    try:
        shutil.rmtree(p, onexc=_fix)                               # 3.12+
    except TypeError:
        shutil.rmtree(p, onerror=_fix)                             # ≤3.11


def _plan(n):
    sp = REPO / "scripts" / "shard_plan.py"
    shards = []
    for k in range(1, n + 1):          # shard_plan 的 --shard 是 1-based
        out = subprocess.run(
            [sys.executable, "-X", "utf8", str(sp), "--plan", str(n),
             "--shard", str(k)],
            capture_output=True, text=True, errors="replace")
        if out.returncode != 0:
            sys.exit(f"PYTEST-SHARDS FAIL: shard_plan --shard {k} -> "
                     f"{out.stderr.strip()[:200]}")
        # shard_plan 把整片文件列表打在一行、空格分隔（绝对路径）
        files = [f for f in out.stdout.split() if f.endswith(".py")]
        if not files:
            sys.exit(f"PYTEST-SHARDS FAIL: shard {k} 空分片")
        try:
            shards.append([str(Path(f).resolve().relative_to(REPO)) for f in files])
        except ValueError:
            sys.exit(f"PYTEST-SHARDS FAIL: 分片文件不在仓内: {files[0]}")
    total = sum(len(s) for s in shards)
    if len({f for s in shards for f in s}) != total:
        sys.exit("PYTEST-SHARDS FAIL: 分片间存在重复文件（覆盖等价破坏）")
    print(f"分片计划：jobs={n} 文件总数={total} 各分片={[len(s) for s in shards]}")
    return shards


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--clone", action="store_true",
                    help="用本地克隆副本跑（已提交 HEAD；默认 in-place 跑工作区）")
    args = ap.parse_args()
    shards = _plan(args.jobs)

    workdirs = [REPO] * args.jobs
    if args.clone:
        workdirs = []
        for k in range(1, args.jobs + 1):
            dest = REPO.parent / f".urx-shard-clone-{os.getpid()}-{k}"
            _rmtree(dest)
            r = subprocess.run(["git", "clone", "--quiet", str(REPO), str(dest)],
                               capture_output=True, text=True, errors="replace")
            if r.returncode != 0:
                sys.exit(f"PYTEST-SHARDS FAIL: clone {dest} -> "
                         f"{r.stderr.strip()[:200]}")
            workdirs.append(dest)

    baseroot = Path(tempfile.gettempdir()) / f"urx-shards-{os.getpid()}"
    procs, t0 = [], time.perf_counter()
    for k, files in enumerate(shards):
        p = subprocess.Popen(
            [sys.executable, "-X", "utf8", "-m", "pytest", *files, "-q", "-l",
             "--tb=short", "--basetemp", str(baseroot / f"s{k + 1}")],
            cwd=str(workdirs[k]), stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True, errors="replace")
        procs.append((k + 1, p))
    results = []
    for k, p in procs:
        out, _ = p.communicate()
        log = baseroot / f"shard{k}.log"
        log.parent.mkdir(exist_ok=True)
        log.write_text(out, encoding="utf-8", errors="replace")
        print(f"  分片输出: {log}")
        tail = out.strip().splitlines()[-1] if out.strip() else "(无输出)"
        results.append((k, p.returncode, tail))
    wall = time.perf_counter() - t0

    for d in workdirs:
        if d != REPO:
            _rmtree(d)

    print(f"墙钟 = {wall:.1f}s（jobs={args.jobs}）")
    for k, rc, tail in results:
        print(f"  shard{k}: exit={rc}  {tail[:80]}")
    bad = [k for k, rc, _ in results if rc != 0]
    if bad:
        print(f"PYTEST-SHARDS FAIL 红分片={bad}（全文见 {baseroot}）")
        sys.exit(2)
    print("PYTEST-SHARDS OK")


if __name__ == "__main__":
    main()
