"""tests/test_s184_memory.py —— 平台失败记忆（方向⑧）：自动草稿 + 去重 + 同库同格式 + 召回。

承重前提：平台写的草稿必须与 `lesson` 工具**同一个库、同一份格式**——否则就是"第二份记忆"
（违"能力只实现一次"）。测试全程把库指向 tmp（**不碰真 `~/.ADV/lessons.jsonl`**）。
"""
import json
import sys

import pytest

import hub_core
import hub_lesson
import hub_runner

PY = sys.executable


@pytest.fixture()
def env(tmp_path, monkeypatch):
    root = tmp_path / "hub"
    pipes = tmp_path / "pipes"
    pipes.mkdir()
    lessons = tmp_path / "lessons.jsonl"
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(root))
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    monkeypatch.setenv("UNIFIED_RX_LESSONS", str(lessons))
    return {"root": root, "pipes": pipes, "lessons": lessons}


def _put(pipes, pid, code):
    (pipes / f"{pid}.json").write_text(json.dumps(
        {"id": pid, "title": pid, "when": "t", "resource_class": "shared", "on": ["manual"],
         "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", code], "timeout_s": 60}]}),
        encoding="utf-8")


def _run(pid):
    pipes, _ = hub_core.load_pipelines()
    return hub_runner.run_pipeline(pipes[pid], actor="test", trigger="chaos")


def test_failed_run_writes_draft_with_ledger_link(env):
    _put(env["pipes"], "adv.bad", "import sys; sys.exit(3)")
    res = _run("adv.bad")
    assert res["verdict"] == "red" and res["lesson"]["status"] == "written"
    rows = [json.loads(ln) for ln in env["lessons"].read_text(encoding="utf-8").splitlines()]
    assert len(rows) == 1
    text = rows[0]["text"]
    assert "adv.bad" in text and res["run_id"] in text and "sig:" in text      # 账本互链
    assert set(rows[0]) == {"id", "text", "ts", "recall_count"}               # 格式同 learn


def test_same_failure_is_deduped(env):
    _put(env["pipes"], "adv.bad", "import sys; sys.exit(3)")
    _run("adv.bad")
    res2 = _run("adv.bad")
    assert res2["lesson"]["status"] == "duplicate"
    assert len(env["lessons"].read_text(encoding="utf-8").strip().splitlines()) == 1


def test_green_run_writes_nothing(env):
    _put(env["pipes"], "adv.ok", "print('ok')")
    res = _run("adv.ok")
    assert res["verdict"] == "green" and res["lesson"]["status"] == "skipped"
    assert not env["lessons"].exists()


def test_lesson_write_failure_does_not_break_run(env, tmp_path, monkeypatch):
    """教训库不可写 ⇒ 运行结果不受影响（lesson 标 error）——记忆不是承重件。"""
    _put(env["pipes"], "adv.bad", "import sys; sys.exit(3)")
    blocker = tmp_path / "nope"
    blocker.write_text("occupied", encoding="utf-8")          # 父路径是文件
    monkeypatch.setenv("UNIFIED_RX_LESSONS", str(blocker / "x" / "lessons.jsonl"))
    res = _run("adv.bad")
    assert res["verdict"] == "red" and res["lesson"]["status"] == "error"
    assert hub_core.verify_chain()["ok"] is True              # 账本完好


def test_recall_reads_platform_draft_in_learn_format(env):
    """跨面：用 learn 的读取器读回平台草稿（同库同格式的硬证据）+ 平台侧召回命中。"""
    _put(env["pipes"], "adv.gate-fast", "import sys; sys.exit(2)")
    _run("adv.gate-fast")
    import tools.learn as learn
    rows = learn._load_lessons(str(env["lessons"]))
    assert rows and "adv.gate-fast" in rows[0]["text"]
    assert set(rows[0]) == {"id", "text", "ts", "recall_count"}
    hits = hub_lesson.recall_for("adv.gate-fast")
    assert hits and "adv.gate-fast" in hits[0]["text"]
