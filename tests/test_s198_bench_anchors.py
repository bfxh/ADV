"""S198 契约：测量锚账门（`scripts/bench_anchor_gate.py` + `spec/bench-anchors.json`）。

由来：H2/H1/H3 的真值树（VF3 等）已随盘迁移消失，但 bench 脚本仍会"跑成功"——
truth 检查全 False、产出格式正常但无意义的数字（量空气）。本门把"锚可用性"入账：
1. **真仓绿**：4 锚在案（vf3/dev-root/yan-agent-src detached、adv-legacy 退役），
   21 处仓外路径引用全部有主；
2. **必红四向**：present 锚目录缺失 / detached 锚的 consumer 缺 ANCHOR-MISSING 出口 /
   未登记的新外部路径引用 / consumer 名实不符——各点名判红；
3. **fail-closed**：登记表缺失 / 坏 JSON / anchors 空 / local_path 理由占位 ⇒ 红；
4. **行为实测**：h2_guard_eval 在缺锚机器上必须退出码 2 且 stderr 点名 ANCHOR-MISSING
   （真路径，不 mock）。
"""
import json
import os
import pathlib
import subprocess
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "bench_anchor_gate.py"
PY = sys.executable


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=300)


def _fixture(tmp_path: pathlib.Path, anchors: list[dict[str, Any]],
              scripts: dict[str, str]) -> tuple[str, str]:
    root = tmp_path / "repo"
    (root / "bench").mkdir(parents=True, exist_ok=True)
    (root / "spec").mkdir(parents=True, exist_ok=True)
    for name, src in scripts.items():
        (root / "bench" / name).write_text(src, encoding="utf-8")
    apath = root / "spec" / "bench-anchors.json"
    apath.write_text(json.dumps({"anchors": anchors, "local_paths": []},
                                ensure_ascii=False), encoding="utf-8")
    return str(root), str(apath)


def _anchor(anchor_id: str, path: str, state: str, consumers: list[str]) -> dict[str, Any]:
    return {"id": anchor_id, "path": path, "state": state, "reason": "夹具",
            "consumers": consumers}


CONSUMER_OK = ('P = r"D:\\data\\gold"\nfrom anchor_guard import require_anchor\n')


# ---------- 真仓自检 ----------

def test_real_repo_green() -> None:
    got = _run()
    assert got.returncode == 0, got.stdout + got.stderr
    assert "BENCH-ANCHOR-GATE OK" in got.stdout and "anchors=4" in got.stdout, got.stdout


def test_registry_shape() -> None:
    data: dict[str, Any] = json.loads((ROOT / "spec" / "bench-anchors.json")
                                      .read_text(encoding="utf-8"))
    states = {a["state"] for a in data["anchors"]}
    assert states <= {"present", "detached", "legacy"}, states
    for a in data["anchors"]:
        assert a["id"] and a["path"] and a["consumers"], a


# ---------- 必红四向 ----------

def test_canary_present_missing_red(tmp_path: pathlib.Path) -> None:
    root, apath = _fixture(
        tmp_path,
        [_anchor("ghost", str(tmp_path / "not-here"), "present", ["bench/c.py"])],
        {"c.py": 'P = r"' + str(tmp_path / "not-here").replace("\\", "/") + '"\n'})
    got = _run("--root", root, "--anchors", apath)
    assert got.returncode != 0 and "present 但目录不存在" in got.stdout, got.stdout


def test_canary_detached_no_exit_red(tmp_path: pathlib.Path) -> None:
    """detached 锚的 consumer 没有 require_anchor 出口 ⇒ 它会静默量空气——必红。"""
    root, apath = _fixture(
        tmp_path,
        [_anchor("gold", "D:/data/gold", "detached", ["bench/c.py"])],
        {"c.py": 'P = r"D:\\data\\gold"\n'})
    got = _run("--root", root, "--anchors", apath)
    assert got.returncode != 0 and "缺 require_anchor 出口" in got.stdout, got.stdout


def test_canary_unregistered_ref_red(tmp_path: pathlib.Path) -> None:
    """新脚本引了未登记的仓外路径 ⇒ 红（主判据：防增量）。"""
    root, apath = _fixture(
        tmp_path,
        [_anchor("gold", "D:/data/gold", "detached", ["bench/c.py"])],
        {"c.py": CONSUMER_OK,
         "new.py": 'E = r"E:\\ghost\\data"\n'})
    got = _run("--root", root, "--anchors", apath)
    assert got.returncode != 0 and "未登记的外部路径引用" in got.stdout \
        and "E:/ghost/data" in got.stdout.replace("\\", "/"), got.stdout


def test_canary_consumer_mismatch_red(tmp_path: pathlib.Path) -> None:
    """登记表宣称 consumer 引用了该锚，实际常量对不上 ⇒ 名实不符必红。"""
    root, apath = _fixture(
        tmp_path,
        [_anchor("gold", "D:/data/gold", "detached", ["bench/c.py"]),
         _anchor("other", "D:/data/other", "detached", ["bench/c.py"])],
        {"c.py": CONSUMER_OK})
    got = _run("--root", root, "--anchors", apath)
    assert got.returncode != 0 and "名实不符" in got.stdout, got.stdout


# ---------- fail-closed ----------

def test_fail_closed_missing_table(tmp_path: pathlib.Path) -> None:
    got = _run("--root", str(ROOT), "--anchors", str(tmp_path / "nope.json"))
    assert got.returncode != 0 and "锚登记表缺失" in got.stdout, got.stdout


def test_fail_closed_bad_json(tmp_path: pathlib.Path) -> None:
    cp = tmp_path / "a.json"
    cp.write_text("{ broken", encoding="utf-8")
    got = _run("--root", str(ROOT), "--anchors", str(cp))
    assert got.returncode != 0 and "坏 JSON" in got.stdout, got.stdout


def test_fail_closed_empty_anchors(tmp_path: pathlib.Path) -> None:
    cp = tmp_path / "a.json"
    cp.write_text(json.dumps({"anchors": []}), encoding="utf-8")
    got = _run("--root", str(ROOT), "--anchors", str(cp))
    assert got.returncode != 0 and "为空" in got.stdout, got.stdout


# ---------- 行为实测（真路径） ----------

def test_h2_missing_anchor_exits_2() -> None:
    """本机器 VF3 缺失：h2_guard_eval 必须 ANCHOR-MISSING 退出码 2，不产数字。"""
    if (pathlib.Path(r"D:\开发\VoxelForge-V3")).is_dir():
        import pytest
        pytest.skip("锚在本机存在——该断言只对缺锚机器成立")
    got = subprocess.run([PY, "-X", "utf8", "bench/h2_guard_eval.py"],
                         capture_output=True, text=True, encoding="utf-8",
                         errors="replace", cwd=str(ROOT), shell=False, timeout=300)
    assert got.returncode == 2, got.stdout + got.stderr
    assert "ANCHOR-MISSING" in got.stderr, got.stderr


def test_importing_bench_modules_has_no_side_effect() -> None:
    """S198 实锤回归：require_anchor 只许在入口调用——tests/swe 会 import bench 模块，
    缺锚机器上 import 必须安静通过（曾因模块级校验把全量 pytest 打成 INTERNALERROR）。"""
    code = ("import sys; sys.path.insert(0, 'bench')"
            "; import ab_run, h2_guard_eval, h3_score, l3_anchor"
            "; print('IMPORT-SAFE')")
    env = {k: v for k, v in os.environ.items() if not k.startswith("UNIFIED_RX_ANCHOR")}
    got = subprocess.run([PY, "-X", "utf8", "-c", code], capture_output=True, text=True,
                         encoding="utf-8", errors="replace", cwd=str(ROOT), env=env,
                         shell=False, timeout=300)
    assert got.returncode == 0 and "IMPORT-SAFE" in got.stdout, got.stdout + got.stderr
