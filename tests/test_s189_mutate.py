"""tests/test_s189_mutate.py —— 突变测试第一片（S189，spec/FRONTIER-CI.md §5.3 第 11 项）判据。

每条都对着模块契约设计，且都能证伪：

1. **金丝雀（两头）**：被测函数的变异**必被杀**（`a+b→a-b`，测试点名）；无测试函数的变异
   **必存活**——`score` 落在 (0,1)，三类变异（算符/常量/边界）都出现。
2. **选测不丢（诚实账）**：杀手测试用 `importlib` **动态导入**（静态闭包看不到它）⇒
   该变异先"存活于选测"、再被**全量复核**杀掉 ⇒ 必须记进 `selection_missed`，
   **不许**混进真存活——这是"选测漏了杀手测试"与"测试真缺口"的分界线。
3. **恢复安全（最重要的契约）**：跑完之后全仓每个文件 sha256 与跑前逐字节一致，
   且无 `.mutbak` 残留。
4. cap 与确定性；未跟踪文件 = 全文件可变异；无变更 = 如实空；CLI JSON 一种形态；
   残留 `.mutbak` 启动即清扫并计入 degraded。
"""
import hashlib
import json
import pathlib
import subprocess
import sys

import hub_mutate

PY = sys.executable
ROOT_HUB_MUTATE = pathlib.Path(hub_mutate.__file__).resolve()

CALC = '''def add(a, b):
    return a + b


def unused(a):
    return a * 2


def clamp(a):
    if a < 10:
        return a
    return 10
'''
TEST_CALC = '''from pkg.calc import add


def test_add():
    assert add(2, 3) == 5
'''
TEST_DYNAMIC = '''import importlib


def test_clamp_via_dynamic_import():
    calc = importlib.import_module("pkg.calc")
    assert calc.clamp(15) == 10
'''


def _git(root: pathlib.Path, *args: str) -> None:
    cp = subprocess.run(["git", "-C", str(root), "-c", "user.name=t", "-c",
                         "user.email=t@t", *args], capture_output=True, shell=False,
                        timeout=60)
    assert cp.returncode == 0, cp.stderr


def _fixture(tmp_path: pathlib.Path, mutate_calc: bool = True) -> pathlib.Path:
    """夹具仓：pkg/calc.py + 两个测试（一个静态可达、一个动态导入静态不可达）。"""
    root = tmp_path / "repo"
    (root / "pkg").mkdir(parents=True)
    (root / "tests").mkdir()
    (root / "conftest.py").write_text("", encoding="utf-8")     # 让 root 进 sys.path
    (root / "pkg" / "calc.py").write_text(CALC, encoding="utf-8")
    (root / "tests" / "test_calc.py").write_text(TEST_CALC, encoding="utf-8")
    (root / "tests" / "test_dynamic.py").write_text(TEST_DYNAMIC, encoding="utf-8")
    _git(root, "init", "-q")
    _git(root, "add", "-A")
    _git(root, "commit", "-qm", "base")
    if mutate_calc:
        touched = (CALC.replace("return a + b", "return a + b  # touched")
                   .replace("return a * 2", "return a * 2  # touched")
                   .replace("if a < 10:", "if a < 10:  # touched")
                   .replace("    return 10", "    return 10  # touched"))
        (root / "pkg" / "calc.py").write_text(touched, encoding="utf-8")
    return root


def _sha_all(root: pathlib.Path) -> dict[str, str]:
    return {p.relative_to(root).as_posix():
            hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*")
            if p.is_file() and "__pycache__" not in p.parts and ".git" not in p.parts
            and not p.name.endswith(hub_mutate.MUTBAK)}


def test_kill_survive_score_and_three_kinds(tmp_path):
    """金丝雀两头：有测试的变异必杀、无测试的必活；三类变异都出现；分数落在 (0,1)。"""
    root = _fixture(tmp_path)
    res = hub_mutate.mutate(root, timeout_s=120)
    assert res["ok"] and res["total"] > 0
    assert res["killed"] >= 1 and res["survived"] >= 2
    assert 0.0 < res["score"] < 1.0
    assert set(res["kinds_generated"]) == {"算符", "常量", "边界"}
    assert all(m["file"] == "pkg/calc.py" for m in res["survivors"])
    old_snippets = " ".join(m["old"] for m in res["survivors"])
    assert "a + b" not in old_snippets                # 被杀的那只不许混进存活名单


def test_selection_missed_is_bookmarked_not_silent(tmp_path):
    """动态 import 的杀手测试静态闭包看不到 ⇒ 全量复核必须把它记进 selection_missed。"""
    root = _fixture(tmp_path)
    res = hub_mutate.mutate(root, timeout_s=120)
    assert res["selection_missed"] >= 1
    assert len(res["selection_missed_list"]) == res["selection_missed"]
    assert all(set(e) >= {"file", "line", "kind"} for e in res["selection_missed_list"])
    missed = json.dumps(res["selection_missed_list"], ensure_ascii=False)
    assert "pkg/calc.py" in missed
    # 分界线：killed 已含 selection_missed；真存活名单必须与 missed 不相交
    assert len(res["survivors"]) == res["survived"]
    assert res["score"] == round(res["killed"] / res["total"], 4)


def test_restore_byte_identical_and_no_bak_left(tmp_path):
    """恢复安全：全仓逐字节一致、无 .mutbak 残留（跑了 ≥7 次改写/恢复之后）。"""
    root = _fixture(tmp_path)
    before = _sha_all(root)
    res = hub_mutate.mutate(root, timeout_s=120)
    assert res["total"] > 0
    assert _sha_all(root) == before
    assert not list(root.rglob(f"*{hub_mutate.MUTBAK}"))


def test_cap_is_deterministic(tmp_path):
    root = _fixture(tmp_path)
    a = hub_mutate.mutate(root, max_mutants=3, timeout_s=120)
    b = hub_mutate.mutate(root, max_mutants=3, timeout_s=120)
    assert a["total"] == 3 and a["capped_from"] > 3
    assert a == b                                      # 同输入同输出（判据可复现）


def test_untracked_file_targets_all_lines(tmp_path):
    """未跟踪文件 = 全文件可变异（没有 diff 就不猜行号——保守口径）。"""
    root = _fixture(tmp_path)
    (root / "pkg" / "extra.py").write_text("def g(a):\n    return a - 1\n", encoding="utf-8")
    res = hub_mutate.mutate(root, timeout_s=120, full_verify=False)
    assert "pkg/extra.py" in res["changed_files"]
    assert res["total"] >= 1


def test_no_changes_is_honest_empty(tmp_path):
    root = _fixture(tmp_path, mutate_calc=False)
    res = hub_mutate.mutate(root, timeout_s=120)
    assert res["total"] == 0 and res["score"] is None and res["changed_files"] == []


def test_stale_mutbak_is_cleaned_and_reported(tmp_path):
    """启动即清扫残留 .mutbak（SIGKILL 情形）——恢复 + 如实计入 degraded，不静默。"""
    root = _fixture(tmp_path, mutate_calc=False)
    (root / "pkg" / "leftover.py").write_text("X = 1\n", encoding="utf-8")
    (root / "pkg" / "leftover.py.mutbak").write_text("X = 1\n", encoding="utf-8")
    res = hub_mutate.mutate(root, timeout_s=120)
    assert any("leftover.py" in d for d in res["degraded"])
    assert not (root / "pkg" / "leftover.py.mutbak").exists()


def test_cli_json_one_shape(tmp_path):
    root = _fixture(tmp_path)
    cp = subprocess.run([PY, "-X", "utf8", str(ROOT_HUB_MUTATE), "--root", str(root),
                         "--no-full-verify"], capture_output=True, text=True,
                        encoding="utf-8", errors="replace", cwd=str(tmp_path), shell=False,
                        timeout=600)
    assert cp.returncode == 0, cp.stdout + cp.stderr
    doc = json.loads(cp.stdout)                        # 一种形态：整段可解析 JSON
    assert {"total", "killed", "survived", "score", "survivors"} <= set(doc)
