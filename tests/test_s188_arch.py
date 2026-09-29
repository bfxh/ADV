"""tests/test_s188_arch.py —— 架构守卫（S188，spec/FRONTIER-CI.md §5.3 第 10 项）判据。

判据先于功能，且每条都能证伪：

1. **金丝雀（门真在扫）**：夹具仓里一条真实违规 ⇒ 门**必红**且**定位到 import 行**；
   修掉 ⇒ 转绿（"先记绿再验红"的照抄纪律）。
2. **scope 语义**：同一 import，函数内（延迟导入）不算 `toplevel` 违例、挪到模块顶部才算
   ——这是对"延迟导入是明文设计"的忠实编码，rule 文件不是摆设。
3. **fail-closed**：规则缺失 / 坏 JSON / 空规则 / 规则缺字段 ⇒ 门判红（守卫被清空即失效）。
4. **渐进启用**：`warn` 规则违规只报告不计红。
5. **真仓自检**：门对本仓自身必须绿（规则必须是"今天已成立"的纪律），且规则 ≥ 3 条
   （防"清空规则换绿"——那等于拆掉守卫还报 OK）。
6. **确定性**：同输入两跑输出逐字相同。
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "arch_gate.py"
RULES = ROOT / "spec" / "arch-rules.json"
PY = sys.executable


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=300)


def _fixture(tmp_path: pathlib.Path, rules: dict) -> tuple[pathlib.Path, pathlib.Path]:
    """夹具仓：pkg/a.py（违规源）+ 独立规则文件（deny pkg.* → import tools）。"""
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("", encoding="utf-8")
    (pkg / "a.py").write_text("import tools\n\n\ndef f():\n    return 1\n", encoding="utf-8")
    rp = tmp_path / "rules.json"
    rp.write_text(json.dumps(rules, ensure_ascii=False), encoding="utf-8")
    return tmp_path, rp


RULES_OK = {"rules": [{"id": "deny-tools", "from": ["pkg", "pkg.*"],
                       "import": ["tools", "tools.*"], "why": "夹具纪律"}]}


def test_canary_violation_is_red_with_line_then_green_after_fix(tmp_path):
    """金丝雀：违规必红 + 定位到行；修掉转绿——门不是"从不红的假门"。"""
    root, rp = _fixture(tmp_path, RULES_OK)
    bad = _run("--root", str(root), "--rules", str(rp))
    assert bad.returncode == 1, bad.stdout + bad.stderr
    assert "pkg.a:1" in bad.stdout and "deny-tools" in bad.stdout
    assert "violations=1" in bad.stdout
    (root / "pkg" / "a.py").write_text("def f():\n    return 1\n", encoding="utf-8")
    good = _run("--root", str(root), "--rules", str(rp))
    assert good.returncode == 0 and "violations=0" in good.stdout, good.stdout


def test_scope_toplevel_distinguishes_lazy_imports(tmp_path):
    """scope=toplevel：函数内延迟导入不算，挪到模块顶部才算——对明文设计的忠实编码。"""
    root, rp = _fixture(tmp_path, {"rules": [{"id": "lt", "from": ["pkg", "pkg.*"],
                                              "import": ["tools", "tools.*"],
                                              "scope": "toplevel"}]})
    (root / "pkg" / "a.py").write_text(
        "def f():\n    import tools\n    return tools\n", encoding="utf-8")
    lazy = _run("--root", str(root), "--rules", str(rp))
    assert lazy.returncode == 0, lazy.stdout
    (root / "pkg" / "a.py").write_text("import tools\n\n\ndef f():\n    return tools\n",
                                       encoding="utf-8")
    eager = _run("--root", str(root), "--rules", str(rp))
    assert eager.returncode == 1 and "pkg.a:1" in eager.stdout, eager.stdout


def test_scope_any_catches_lazy_imports_too(tmp_path):
    """scope 缺省 any：函数内也抓（hub 底座分层规则就靠这个）。"""
    root, rp = _fixture(tmp_path, RULES_OK)
    (root / "pkg" / "a.py").write_text(
        "def f():\n    from tools import anything\n    return anything\n", encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(rp))
    assert got.returncode == 1 and "pkg.a:2" in got.stdout, got.stdout


def test_relative_import_resolved(tmp_path):
    """`from . import x` 就地解析成包名后匹配（工具面内部相对导入是常态）。"""
    root, rp = _fixture(tmp_path, {"rules": [{"id": "self-dep", "from": ["pkg.a"],
                                              "import": ["pkg.b"]}]})
    (root / "pkg" / "b.py").write_text("X = 1\n", encoding="utf-8")
    (root / "pkg" / "a.py").write_text("from .b import X\n\n\ndef f():\n    return X\n",
                                       encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(rp))
    assert got.returncode == 1 and "import pkg.b" in got.stdout, got.stdout


def test_allow_and_warn_tiers(tmp_path):
    root, rp = _fixture(tmp_path, {"rules": [
        {"id": "deny-tools", "from": ["pkg", "pkg.*"], "import": ["tools", "tools.*"],
         "allow": ["pkg.a"]},
        {"id": "warn-only", "from": ["pkg", "pkg.*"], "import": ["os"], "warn": True}]})
    (root / "pkg" / "a.py").write_text("import os\nimport tools\n\n\ndef f():\n    return 1\n",
                                       encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(rp))
    assert got.returncode == 0, got.stdout                 # 豁免 + warn ⇒ 不红
    assert "WARN" in got.stdout and "warn-only" in got.stdout


def test_fail_closed_on_unhealthy_rules(tmp_path):
    """规则缺失 / 坏 JSON / 空规则 / 缺字段 ⇒ 门判红（fail-closed，不静默放行）。"""
    root, _rp = _fixture(tmp_path, RULES_OK)
    missing = _run("--root", str(root), "--rules", str(tmp_path / "nope.json"))
    assert missing.returncode == 1 and "规则不健康" in missing.stdout
    broken = tmp_path / "broken.json"
    broken.write_text("{not json", encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(broken))
    assert got.returncode == 1 and "坏 JSON" in got.stdout
    empty = tmp_path / "empty.json"
    empty.write_text(json.dumps({"rules": []}), encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(empty))
    assert got.returncode == 1 and "非空数组" in got.stdout
    incomplete = tmp_path / "incomplete.json"
    incomplete.write_text(json.dumps({"rules": [{"id": "x"}]}), encoding="utf-8")
    got = _run("--root", str(root), "--rules", str(incomplete))
    assert got.returncode == 1 and "缺字段" in got.stdout


def test_real_repo_is_green_and_rules_are_not_gutted():
    """真仓自检：门对本仓绿（规则=今天已成立的纪律），且规则 ≥ 3 条（防清空换绿）。"""
    got = _run()
    assert got.returncode == 0, got.stdout + got.stderr
    assert "violations=0" in got.stdout
    doc = json.loads(RULES.read_text(encoding="utf-8"))
    assert len(doc["rules"]) >= 3
    assert all(r.get("id") and r.get("why") for r in doc["rules"])


def test_deterministic_output(tmp_path):
    root, rp = _fixture(tmp_path, RULES_OK)
    a = _run("--root", str(root), "--rules", str(rp))
    b = _run("--root", str(root), "--rules", str(rp))
    assert (a.stdout, a.returncode) == (b.stdout, b.returncode)
