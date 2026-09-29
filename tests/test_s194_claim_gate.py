"""S194 契约：主张可复算门（`scripts/claim_gate.py` + `spec/claim-checks.json`）。

由来（用户 2026-09-29 口径）："把过程变成东西 / 关系变属性 / 条件变本质 / 局部变总体 /
解释变终点"——这类表述幻觉**在散文层不可靠地可判**（本片四轮测量：字面词表版假阳性主导）。
可判的那一半是结构：**当时的观察被固化成文档事实，然后随代码漂移**。本文件把那一半钉住。

判据两头都有：
1. **真仓自检**：登记表里每条主张对当前仓必须成立（走 `--root` 真路径，非 import 内部函数）；
2. **金丝雀红**：夹具里故意改错**文档数字** ⇒ 必红且点名该条；改错**真值源**同样必红；
   文案改了导致**主张找不到**也必红（"账没了"与"账错了"同罪）；
3. **fail-closed**：登记表缺失 / 坏 JSON / 空登记 / 条目缺字段 ⇒ 红；
4. **探针自洽**：`steps:` 真值源与 `local_gate.STEPS` 逐档位一致（防"解析悄悄数错"——
   本片就踩过：`literal_eval` 元组失败被吞成 0，门判红才发现）。
"""
import importlib.util
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "claim_gate.py"
PY = sys.executable


def _load(name: str, rel: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / rel)
    assert spec is not None and spec.loader is not None, f"加载失败: {rel}"
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=600)


def _fixture(tmp_path: pathlib.Path, doc_num: int, truth: int) -> tuple[pathlib.Path, pathlib.Path]:
    """夹具仓：一个数字真值文件（json_len 源）+ 一份声称 `doc_num 件` 的文档 + 合规账目契约。

    账目契约是**必填**（fail-closed 的第二族判据）——夹具必须给一份合规的，否则红在契约上、
    就测不到第一族的比对逻辑了。
    """
    (tmp_path / "spec").mkdir(parents=True, exist_ok=True)
    (tmp_path / "truth.json").write_text(
        json.dumps({"items": [{"n": i} for i in range(truth)]}), encoding="utf-8")
    (tmp_path / "DOC.md").write_text(f"工具面共 {doc_num} 件。\n", encoding="utf-8")
    (tmp_path / "spec" / "ROUNDLOG.md").write_text(
        "## S200 · 夹具条目\n- 决策：夹具\n- 证据：本文件\n", encoding="utf-8")
    claims = {"claims": [{"id": "fx-count", "file": "DOC.md",
                          "pattern": "共 (\\d+) 件", "expect": ["json_len:truth.json:items"],
                          "why": "夹具"}],
              "entry_contract": {"file": "spec/ROUNDLOG.md", "require": ["决策：", "证据："],
                                 "min_session": 180, "why": "夹具"}}
    cp = tmp_path / "claims.json"
    cp.write_text(json.dumps(claims, ensure_ascii=False), encoding="utf-8")
    return tmp_path, cp


# ---------- 真仓自检 ----------

def test_real_repo_claims_hold():
    """真路径：默认登记表逐条复算 ⇒ 绿（文档数字与真值源一致）。"""
    got = _run()
    assert got.returncode == 0, got.stdout + got.stderr
    assert "CLAIM-GATE OK" in got.stdout and "mismatch=0" in got.stdout, got.stdout


def test_registry_entries_point_at_real_files():
    """每条登记指向的**文档必须存在**（登记表写错路径 = 门扫空气）。"""
    claims = json.loads((ROOT / "spec" / "claim-checks.json").read_text(encoding="utf-8"))["claims"]
    assert len(claims) >= 5, "登记表被清空到只剩几条？"
    for c in claims:
        assert (ROOT / c["file"]).is_file(), c


def test_steps_probe_agrees_with_steps_constant():
    """探针自洽：`steps:` 与 local_gate.STEPS 的档位计数逐项一致（本片踩过数错为 0）。"""
    cg = _load("cg_steps", "scripts/claim_gate.py")
    lg = _load("lg_steps", "scripts/local_gate.py")
    steps = lg.STEPS
    want = {"total": len(steps),
            "fast": sum(1 for s in steps if s[2] == "fast"),
            "full": sum(1 for s in steps if s[2] in ("fast", "full"))}
    assert cg._steps_counts(ROOT) == want, (cg._steps_counts(ROOT), want)


# ---------- 金丝雀：两头都要能红 ----------

def test_canary_wrong_doc_number_is_red(tmp_path):
    """金丝雀红①：文档数字错了 ⇒ 必红且点名该条（给真值便于对账）。"""
    root, cp = _fixture(tmp_path, doc_num=9, truth=5)
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1, got.stdout
    assert "fx-count" in got.stdout and "主张 [9] ≠ 真值 [5]" in got.stdout, got.stdout


def test_canary_truth_drift_is_red(tmp_path):
    """金丝雀红②：**真值变了**（文档没跟上）⇒ 同样必红——这才是"漂移"形态。"""
    root, cp = _fixture(tmp_path, doc_num=5, truth=5)
    assert _run("--root", str(root), "--claims", str(cp)).returncode == 0
    (root / "truth.json").write_text(
        json.dumps({"items": [{"n": i} for i in range(7)]}), encoding="utf-8")
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1 and "真值 [7]" in got.stdout, got.stdout


def test_canary_missing_claim_is_red(tmp_path):
    """金丝雀红③：文案改了、登记表没跟 ⇒ "主张找不到"也判红（账没了≠没账）。"""
    root, cp = _fixture(tmp_path, doc_num=5, truth=5)
    (root / "DOC.md").write_text("工具面共五件（数字被拿掉了）。\n", encoding="utf-8")
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1 and "主张找不到" in got.stdout, got.stdout


def test_canary_unknown_truth_source_is_red(tmp_path):
    """未知真值源 ⇒ 红且说明（不许静默跳过一条主张）。"""
    root, _ = _fixture(tmp_path, doc_num=5, truth=5)
    cp = tmp_path / "bad.json"
    cp.write_text(json.dumps({
        "claims": [{"id": "x", "file": "DOC.md", "pattern": "共 (\\d+) 件",
                    "expect": ["nope:whatever"], "why": "w"}],
        "entry_contract": {"file": "spec/ROUNDLOG.md", "require": ["决策："],
                           "min_session": 180, "why": "w"}},
        ensure_ascii=False), encoding="utf-8")
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1 and "未知真值源" in got.stdout, got.stdout


# ---------- fail-closed ----------

def test_fail_closed_registry_states(tmp_path):
    """缺失 / 坏 JSON / 空登记 / 缺字段 ⇒ 四种都红（门被清空即失效）。"""
    root, _cp = _fixture(tmp_path, doc_num=5, truth=5)
    cases = {
        "missing": tmp_path / "nope.json",
        "bad_json": tmp_path / "bad.json",
        "empty": tmp_path / "empty.json",
        "no_field": tmp_path / "nofield.json",
    }
    cases["bad_json"].write_text("{ not json", encoding="utf-8")
    cases["empty"].write_text(json.dumps({"claims": []}), encoding="utf-8")
    cases["no_field"].write_text(json.dumps({"claims": [{"id": "x"}]}), encoding="utf-8")
    for name, path in cases.items():
        got = _run("--root", str(root), "--claims", str(path))
        assert got.returncode == 1, f"{name} 必须红：{got.stdout}"
        assert "登记表不健康" in got.stdout, (name, got.stdout)


# ---------- 第二族判据：账目格式契约（治「把解释变成终点」）----------

EC = {"file": "spec/ROUNDLOG.md", "require": ["决策：", "证据："], "min_session": 180,
      "why": "夹具"}


def _entry_fixture(tmp_path: pathlib.Path, log_text: str, ec: dict | None = None):
    """账目夹具：一份 claims（满足第一族即可）+ 一份 ROUNDLOG 文本 + entry_contract。"""
    (tmp_path / "spec").mkdir(parents=True, exist_ok=True)
    (tmp_path / "spec" / "ROUNDLOG.md").write_text(log_text, encoding="utf-8")
    (tmp_path / "truth.json").write_text(json.dumps({"items": [1]}), encoding="utf-8")
    (tmp_path / "DOC.md").write_text("共 1 件。\n", encoding="utf-8")
    doc: dict = {"claims": [{"id": "fx", "file": "DOC.md", "pattern": "共 (\\d+) 件",
                             "expect": ["json_len:truth.json:items"], "why": "夹具"}]}
    if ec is not None:
        doc["entry_contract"] = ec
    cp = tmp_path / "claims.json"
    cp.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return tmp_path, cp


OK_ENTRY = "## S200 · 齐栏条目\n- 决策：做 A\n- 证据：见 `x.py`\n"
OLD_ENTRY = "## S100 · 老条目\n- 决策：x\n"


def test_entry_contract_real_repo_passes_and_is_reported():
    """真仓：账目契约在跑（摘要里看得见闸门与栏位），且 S180+ 条目齐栏。"""
    got = _run()
    assert got.returncode == 0 and "entry>=180" in got.stdout, got.stdout


def test_entry_contract_canary_missing_evidence_is_red(tmp_path):
    """金丝雀红：新条目只写归因、没有证据 ⇒ 必红并点名条目号。"""
    root, cp = _entry_fixture(
        tmp_path, OK_ENTRY + "\n## S999 · 只写归因\n- 决策：一句话\n- 备注：根因是它。\n", EC)
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1 and "S999" in got.stdout and "证据：" in got.stdout, got.stdout
    assert "S200" not in got.stdout, "齐栏条目不得被牵连"


def test_entry_contract_grandfathers_history(tmp_path):
    """棘轮钉现状：min_session 之前的条目缺证据也不红（历史不追溯）。"""
    root, cp = _entry_fixture(tmp_path, OLD_ENTRY + "\n" + OK_ENTRY, EC)
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 0, got.stdout


def test_entry_contract_fail_closed_states(tmp_path):
    """契约缺失 / require 空 / min_session 不是整数 / 账目里一条条目都没有 ⇒ 四种都红。"""
    root, cp = _entry_fixture(tmp_path, OK_ENTRY, ec=None)
    assert _run("--root", str(root), "--claims", str(cp)).returncode == 1, "缺 entry_contract 必红"
    for bad_ec in ({"file": "spec/ROUNDLOG.md", "require": [], "min_session": 180},
                   {"file": "spec/ROUNDLOG.md", "require": ["决策："], "min_session": "180"},
                   {"file": "spec/ROUNDLOG.md", "require": ["决策："]}):
        root2, cp2 = _entry_fixture(tmp_path / f"c{abs(hash(str(bad_ec))) % 9999}",
                                    OK_ENTRY, bad_ec)
        got = _run("--root", str(root2), "--claims", str(cp2))
        assert got.returncode == 1 and "登记表不健康" in got.stdout, (bad_ec, got.stdout)
    root3, cp3 = _entry_fixture(tmp_path / "noentry", "（这里没有条目）\n", EC)
    got3 = _run("--root", str(root3), "--claims", str(cp3))
    assert got3.returncode == 1 and "账没了" in got3.stdout, got3.stdout


def test_entry_contract_missing_file_is_red(tmp_path):
    """契约指向的账目文件不存在 ⇒ 红（契约指向空气）。"""
    root, cp = _entry_fixture(tmp_path, OK_ENTRY, EC)
    (root / "spec" / "ROUNDLOG.md").unlink()
    got = _run("--root", str(root), "--claims", str(cp))
    assert got.returncode == 1 and "账目不在" in got.stdout, got.stdout
