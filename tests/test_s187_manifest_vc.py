"""tests/test_s187_manifest_vc.py —— S187 两片的判据单测（spec/FRONTIER-CI.md 方向③⑤）。

覆盖：
- **③ manifest 声明**：认合法 / 拒非法（8 类反例）/ 老 manifest 不破（向后兼容）/
  依赖只做存在性（"缺 id"与"版本未解析"如实分列，不假装校验过）；
- **⑤ 构建凭证**：形状齐 / **账本行纯函数**（跨进程重算逐位同）/ 篡改必变 /
  轨迹与封条同口径 / 签名档位如实 `unsigned`；
- **两条回归**（S187 实测缺陷，都是"判据自己骗自己"）：
  ① `--seal` 复核入口曾用 S177 前的旧键 `log_sha256` 重算 ⇒ **现行运行全假红**
     （本文件用"新 schema + 旧 schema 两代行"把两代都钉住）；
  ② 门里 `auth-matrix` 的"MCP 无授权必拒"曾在**空工具面**下空过
     （未知工具也回 ok=False ⇒ 断言恒真）；用 hub_gate 自己的导入上下文验。
"""
import json
import pathlib
import subprocess
import sys

import pytest

import hub_core
import hub_manifest
import hub_runner
import hub_vc

PY = sys.executable
ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "hub_gate.py"

BAD_DECLARATIONS = (
    {"version": "v1.2"},                 # 前缀不是数字
    {"version": "1.2.3.4.5"},            # 段数超限
    {"version": 12},                     # 不是字符串
    {"requires": "adv.x"},               # 不是列表
    {"requires": ["Adv.X"]},             # id 大写
    {"requires": ["adv.meta"]},          # 自引用
    {"requires": ["adv.x", "adv.x"]},    # 重复
    {"requires": ["adv.x@v1"]},          # 版本前缀不是数字
)


@pytest.fixture()
def hub_root(tmp_path, monkeypatch):
    """隔离数据根 + 管线目录（不触真实 ~/.ADV/hub）。"""
    root = tmp_path / "hub"
    pipes = tmp_path / "pipes"
    pipes.mkdir(parents=True, exist_ok=True)
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(root))
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    return {"root": root, "pipes": pipes}


def _manifest(pid, code, **kw):
    m = {"id": pid, "title": pid, "when": "t", "resource_class": "shared",
         "on": ["manual"],
         "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", code], "timeout_s": 60}]}
    m.update(kw)
    return m


def _put(pipes: pathlib.Path, pid: str, code: str, **kw) -> None:
    (pipes / f"{pid}.json").write_text(json.dumps(_manifest(pid, code, **kw)),
                                       encoding="utf-8")


def _gate_cli(*args: str) -> subprocess.CompletedProcess:
    """子进程跑门的复核入口（env 继承 ⇒ 看得到隔离数据根）。"""
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          cwd=str(ROOT), shell=False, timeout=180)


# ---------------- ③ manifest 声明字段 ----------------

def test_manifest_accepts_version_and_requires():
    hub_core.validate_manifest(
        "adv.meta.json",
        _manifest("adv.meta", "print(1)", version="1.2.3",
                  requires=["adv.other", "adv.two@1.0", "adv.plain@1"]))


@pytest.mark.parametrize("patch", BAD_DECLARATIONS)
def test_manifest_rejects_bad_declarations(patch):
    """非法声明必须被拒——否则"字段被认"可以只是收下就完事。"""
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.meta.json", _manifest("adv.meta", "print(1)", **patch))


def test_manifest_backward_compatible_without_declarations(hub_root):
    """老 manifest（没有 version/requires）照旧加载——新字段是**可选**的。"""
    _put(hub_root["pipes"], "adv.old", "print(1)")
    pipes, invalid = hub_core.load_pipelines()
    assert not invalid and pipes["adv.old"].get("version") is None
    assert hub_manifest.missing_requires(pipes) == {}


def test_load_pipelines_carries_declared_fields(hub_root):
    _put(hub_root["pipes"], "adv.meta", "print(1)", version="1.0",
         requires=["adv.nobody@1.0"])
    pipes, invalid = hub_core.load_pipelines()
    assert not invalid
    assert pipes["adv.meta"]["version"] == "1.0"
    assert hub_core.missing_requires(pipes)["adv.meta"] == ["adv.nobody@1.0（缺 adv.nobody）"]


def test_missing_requires_separates_missing_from_unresolved():
    """缺 id 与"版本未解析"必须**分列**：后者是我们**不做**的事，不能报成"已校验"。"""
    gaps = hub_core.missing_requires(
        {"adv.a": {"requires": ["adv.b@1.0", "adv.gone", "adv.b"]}, "adv.b": {}})
    assert gaps["adv.a"] == ["adv.b@1.0（版本未解析）", "adv.gone（缺 adv.gone）"]
    assert "adv.b" not in gaps


def test_hub_pipelines_surfaces_declarations(hub_root):
    import tools.hub as hubtools
    _put(hub_root["pipes"], "adv.meta", "print(1)", version="2.1", requires=["adv.two@1.0"])
    _put(hub_root["pipes"], "adv.two", "print(1)")
    got = hubtools.hub_pipelines()
    row = next(r for r in got["pipelines"] if r["id"] == "adv.meta")
    assert row["version"] == "2.1" and row["requires"] == ["adv.two@1.0"]
    assert got["missing_requires"] == {"adv.meta": ["adv.two@1.0（版本未解析）"]}


# ---------------- ⑤ 构建凭证 ----------------

def _green_row(hub_root) -> dict:
    _put(hub_root["pipes"], "adv.vc", "print('vc')")
    pipes, _inv = hub_core.load_pipelines()
    res = hub_runner.run_pipeline(pipes["adv.vc"], actor="cli", trigger="manual")
    assert res["verdict"] == "green"
    return hub_vc.final_row(res["run_id"])


def test_credential_shape_and_purity(hub_root):
    """形状齐 + **账本行纯函数**：独立进程重算的摘要必须与本进程逐位相同。"""
    row = _green_row(hub_root)
    cred = hub_vc.build(row)
    assert cred["type"] == hub_vc.CRED_TYPE and cred["spec"] == hub_vc.CRED_SPEC
    assert cred["sha256"] == hub_vc.digest(cred)
    subj = cred["subject"]
    assert subj["run_id"] == row["id"] and subj["verdict"] == "green"
    assert subj["source"]["git_head"] == row["fingerprint"]["git_head"]
    assert subj["environment"]["python"] and subj["environment"]["env_keys_hash"]
    assert cred["ledger"]["hash"] == row["hash"] and cred["seal"] == row["seal"]
    assert cred["issuer"]["actor"] == "cli" and cred["issued_at"] == row["ts"]
    cp = _gate_cli("--credential", row["id"])
    assert cp.returncode == 0 and cred["sha256"] in cp.stdout, cp.stdout + cp.stderr


def test_credential_is_honestly_unsigned(hub_root):
    """签名档位如实：**不许**假装签过（纯 stdlib 无 Ed25519）。"""
    cred = hub_vc.build(_green_row(hub_root))
    assert cred["proof"] == hub_vc.PROOF
    assert cred["proof"]["type"] == "none" and cred["proof"]["cryptosuite"] == "unsigned"
    assert "不是签名" in cred["proof"]["reason"]


def test_credential_changes_when_any_field_tampered(hub_root):
    row = _green_row(hub_root)
    base = hub_vc.build(row)["sha256"]
    for patch in ({"verdict": "hacked"}, {"ts": 1}, {"actor": "someone-else"},
                  {"seal": "0" * 64}):
        assert hub_vc.build({**row, **patch})["sha256"] != base, patch
    steps = [dict(row["steps"][0], exit=99)]
    assert hub_vc.build({**row, "steps": steps})["sha256"] != base


def test_credential_trace_matches_seal_payload(hub_root):
    """轨迹字段必须与封印载荷同一口径——两边分叉，凭证上的读数就不是封条盖住的读数。"""
    row = _green_row(hub_root)
    cred = hub_vc.build(row)
    assert cred["subject"]["trace"] == hub_vc.seal_payload(row)["steps"]
    assert set(cred["subject"]["trace"][0]) == set(hub_vc.TRACE_KEYS)


def test_for_rows_skips_unsealed_and_reports_build_errors(hub_root):
    row = _green_row(hub_root)
    got = hub_vc.for_rows([{"id": "r-start", "phase": "start"}, row])
    assert set(got) == {row["id"]}
    broken = hub_vc.for_rows([{**row, "ts": "not-a-number"}])
    assert broken[row["id"]]["ok"] is False        # 坏凭证不许被当成"不存在"


def test_hub_runs_surfaces_credentials(hub_root):
    import tools.hub as hubtools
    row = _green_row(hub_root)
    got = hubtools.hub_runs(5)
    assert got["credentials"][row["id"]] == hub_vc.build(row)


# ---------------- 回归：两代封条的复核入口 ----------------

def _legacy_row(run_id: str) -> dict:
    """S177 前的账本行：单流日志 ⇒ 步骤字段是 `log_sha256`（没有 out/err 之分）。"""
    step = {"step": "s", "exit": 0, "log_sha256": "a" * 64, "log_bytes": 3}
    fp = {"manifest_sha256": "m" * 64, "git_head": "h" * 40, "git_dirty": False}
    payload = {"pipeline": "adv.legacy", "manifest_sha256": fp["manifest_sha256"],
               "git_head": fp["git_head"], "git_dirty": fp["git_dirty"],
               "verdict": "green",
               "steps": [{k: step.get(k) for k in ("step", "exit", "log_sha256")}]}
    return {"id": run_id, "phase": "final", "verdict": "green", "pipeline": "adv.legacy",
            "actor": "cli", "trigger": "manual", "fingerprint": fp, "steps": [step],
            "seal": hub_runner.seal_of(payload), "ts": 1700000000,
            "hash": "c" * 64, "prev_hash": "d" * 64}


def test_seal_cli_verifies_current_schema_row(hub_root):
    row = _green_row(hub_root)
    cp = _gate_cli("--seal", row["id"])
    assert cp.returncode == 0 and "OK" in cp.stdout, cp.stdout + cp.stderr


def test_seal_cli_verifies_legacy_row_without_false_red(hub_root):
    """**回归（缺陷 A）**：复核必须按**记录代次**取键。

    修前一律用旧键 `log_sha256` 重算 ⇒ 现行（新 schema）行全部假红；反过来若一律用新键，
    则这条老行会假红。两代都要能复核——所以两代各钉一条。
    """
    row = _legacy_row("r-legacy-0001")
    p = hub_core.runs_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(row, ensure_ascii=False) + "\n", encoding="utf-8")
    cp = _gate_cli("--seal", "r-legacy-0001")
    assert cp.returncode == 0 and "OK" in cp.stdout, cp.stdout + cp.stderr
    assert hub_vc.seal_payload(row)["steps"][0] == {"step": "s", "exit": 0,
                                                    "log_sha256": "a" * 64}


def test_gate_auth_matrix_is_not_vacuous():
    """**回归（缺陷 B）**：在门自己的导入上下文里，"MCP 无授权必拒"必须真跑到工具。

    空工具面下未知工具同样回 `ok=False` ⇒ 那条断言恒真（假绿）。判据里已把"注册面非空"
    一并纳入；这里从**外部**再钉一次，防它退回去。
    """
    code = ("import sys, pathlib;"
            "sys.path.insert(0, str(pathlib.Path(r'%s') / 'scripts'));"
            "import hub_gate;"
            "ok, detail = hub_gate._auth_matrix();"
            "print('TRUE' if ok else 'FALSE');"
            "print(detail)" % ROOT)
    cp = subprocess.run([PY, "-X", "utf8", "-c", code], capture_output=True, text=True,
                        encoding="utf-8", errors="replace", cwd=str(ROOT), shell=False,
                        timeout=180)
    assert cp.stdout.splitlines()[0] == "TRUE", cp.stdout + cp.stderr
    detail = cp.stdout.splitlines()[1]
    assert "registry真实=True(n=" in detail and "MCP 无授权拒=True" in detail, detail


def test_gate_checks_include_s187_rows():
    """两条新判据必须真的挂在门的判据表里（不是只写了函数）。"""
    src = GATE.read_text(encoding="utf-8")
    for name in ("manifest-version-requires", "credential-vc"):
        assert f'("{name}"' in src, f"{name} 未挂进 _checks()"
