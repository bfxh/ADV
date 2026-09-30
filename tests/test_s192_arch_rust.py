"""tests/test_s192_arch_rust.py —— 架构守卫的 **Rust 语言面**判据（S192）。

动机（用户项目）：BSHSQ 的 `Cargo.toml` 顶部用注释写着依赖 DAG 与"禁止环"，而**没有任何
机器在守它**；VoxelForge 同族（渲染/逻辑/仿真分层靠自觉）。本片让 `arch_gate.py` 也认
`.rs` 的 `use` 边，于是"某层不许依赖某层"在 Rust 工程里同样可声明、可定位到行。

判据原则同 S188：每条都能证伪，且**两头都有**——
1. **金丝雀红**：Rust 夹具里一条真违规 ⇒ 门必红且**定位到 `use` 行**；修掉 ⇒ 转绿；
2. **金丝雀静**：正确分层的夹具 ⇒ 一条都不报（只证"能报"会放过"见谁都报"的假门）；
3. **解析表**：分组/嵌套分组/glob/别名/多行 `use`，`crate::`/`self::`/`super::` 就地解析成
   crate 限定名，模块名（含 `lib.rs`/`main.rs`/`mod.rs` 折叠与 `src/bin` 处理）逐条钉住；
4. **连字符/下划线双形态**：crate 名写 `vxl-phys-core`、路径写 `vxl_phys_core`，规则写哪个都命中；
5. **scope=toplevel**：函数体内的 `use`（延迟导入）不算，挪到模块顶部才算；
6. **混语言**：同一根下 `.py` 与 `.rs` 的边都在（Python 语义不变——S188 的判据另行继续跑）；
7. **确定性**：同输入两跑输出逐字相同。
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "arch_gate.py"
PY = sys.executable


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=300)


def _ws(tmp_path: pathlib.Path, crate: str = "vxl-phys-solver") -> pathlib.Path:
    """最小可识别工作区：一个 crate + 它的 Cargo.toml（模块名要靠它才成 crate 限定名）。"""
    cdir = tmp_path / "crates" / crate
    (cdir / "src").mkdir(parents=True)
    (cdir / "Cargo.toml").write_text(
        f'[package]\nname = "{crate}"\nversion = "0.1.0"\n', encoding="utf-8")
    (tmp_path / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
    return cdir / "src"


def _rules(tmp_path: pathlib.Path, rules: list[dict]) -> pathlib.Path:
    rp = tmp_path / "rules.json"
    rp.write_text(json.dumps({"rules": rules}, ensure_ascii=False), encoding="utf-8")
    return rp


SOLVER_NO_RENDER = {
    "id": "solver-no-render", "from": ["vxl-phys-solver", "vxl-phys-solver.*"],
    "import": ["vxl-phys-render"],       # 单模式：重叠模式会让同一行被重复报（已知缺陷，另案）
    "why": "求解器不得依赖渲染层（夹具纪律）",
}


def test_canary_rust_violation_is_red_with_use_line(tmp_path):
    """金丝雀红：`use vxl_phys_render::…` ⇒ 必红且行号指向那条 use；删掉转绿。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text(
        "use vxl_phys_core::Vec3;\n\npub fn step() -> Vec3 {\n    Vec3::ZERO\n}\n",
        encoding="utf-8")
    rp = _rules(tmp_path, [SOLVER_NO_RENDER])
    clean = _run("--root", str(tmp_path), "--rules", str(rp))
    assert clean.returncode == 0 and "violations=0" in clean.stdout, clean.stdout
    assert "unparsed=0" in clean.stdout, clean.stdout

    (src / "sim.rs").write_text(
        "use vxl_phys_core::Vec3;\nuse vxl_phys_render::Sprite;\n\n"
        "pub fn step() -> Vec3 {\n    Vec3::ZERO\n}\n", encoding="utf-8")
    bad = _run("--root", str(tmp_path), "--rules", str(rp))
    assert bad.returncode == 1, bad.stdout + bad.stderr
    assert "vxl-phys-solver.sim:2" in bad.stdout, bad.stdout
    assert "solver-no-render" in bad.stdout and "violations=1" in bad.stdout, bad.stdout


def test_canary_quiet_upper_layer_may_use_lower(tmp_path):
    """金丝雀静：上层依赖下层是正常方向 ⇒ 一条都不报。"""
    src = _ws(tmp_path, "vxl-phys-render")
    (src / "draw.rs").write_text(
        "use vxl_phys_solver::sim::Step;\n\npub fn draw(s: Step) {}\n", encoding="utf-8")
    rp = _rules(tmp_path, [SOLVER_NO_RENDER])
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert got.returncode == 0 and "violations=0" in got.stdout, got.stdout


def test_scope_toplevel_rust_fn_body_is_lazy_import(tmp_path):
    """函数体内的 use = 延迟导入：toplevel 规则不管，scope=any 才管（与 Python 同语义）。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text(
        "pub fn step() {\n    use vxl_phys_render::Sprite;\n    let _ = Sprite;\n}\n",
        encoding="utf-8")
    top = _rules(tmp_path, [dict(SOLVER_NO_RENDER, scope="toplevel")])
    got = _run("--root", str(tmp_path), "--rules", str(top))
    assert got.returncode == 0 and "violations=0" in got.stdout, got.stdout
    any_scope = _rules(tmp_path, [SOLVER_NO_RENDER])
    got2 = _run("--root", str(tmp_path), "--rules", str(any_scope))
    assert got2.returncode == 1 and "vxl-phys-solver.sim:2" in got2.stdout, got2.stdout


def test_hyphen_and_underscore_crate_names_both_match(tmp_path):
    """crate 名 `vxl-phys-render` 与 Rust 路径 `vxl_phys_render` 都要能被规则命中。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text("use vxl_phys_render::Sprite;\n", encoding="utf-8")
    for pat in ("vxl-phys-render", "vxl_phys_render"):
        rp = _rules(tmp_path, [dict(SOLVER_NO_RENDER, **{"import": [pat]})])
        got = _run("--root", str(tmp_path), "--rules", str(rp))
        assert got.returncode == 1, f"{pat} 未命中：{got.stdout}"


def test_use_forms_expand_to_expected_candidates(tmp_path):
    """分组/嵌套分组/别名/glob/多行 use 的展开逐条钉住（候选含点前缀）。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text(
        "use vxl_phys_render::{Sprite, atlas as atlas_mod};\n"
        "use vxl_phys_render::prelude::*;\n"
        "use vxl_phys_core::{\n    Vec3,\n    geom::{Sphere, Aabb},\n};\n",
        encoding="utf-8")
    rp = _rules(tmp_path, [
        {"id": "r-sprite", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_render.Sprite"]},
        {"id": "r-atlas", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_render.atlas"]},
        {"id": "r-pre", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_render.prelude"]},
        {"id": "r-sphere", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_core.geom.Sphere"]},
        {"id": "r-aabb", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_core.geom.Aabb"]},
    ])
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert got.returncode == 1, got.stdout
    for rid in ("r-sprite", "r-atlas", "r-pre", "r-sphere", "r-aabb"):
        assert rid in got.stdout, f"{rid} 未命中：{got.stdout}"
    # 行号：前两条 use 在 1/2 行，分组那条起于 3 行（多行 use 记**起始行**）
    assert "vxl-phys-solver.sim:1" in got.stdout and "vxl-phys-solver.sim:2" in got.stdout, got.stdout
    assert "vxl-phys-solver.sim:3" in got.stdout, got.stdout


def test_module_name_table(tmp_path):
    """模块名规则：crate 限定 + lib/main/mod 折叠 + src/bin 处理 + 无 Cargo.toml 回退。"""
    src = _ws(tmp_path, "vxl-phys-core")
    (src / "lib.rs").write_text("pub mod geom;\n", encoding="utf-8")
    (src / "geom").mkdir()
    (src / "geom" / "mod.rs").write_text("", encoding="utf-8")
    (src / "geom" / "sphere.rs").write_text("use vxl_phys_render::Sprite;\n", encoding="utf-8")
    (src / "bin").mkdir()
    (src / "bin" / "demo.rs").write_text("use vxl_phys_render::Sprite;\n", encoding="utf-8")
    rp = _rules(tmp_path, [{"id": "r", "from": ["*"], "import": ["vxl_phys_render.Sprite"]}])
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert "vxl-phys-core.geom.sphere:1" in got.stdout, got.stdout
    assert "vxl-phys-core.demo:1" in got.stdout, got.stdout      # src/bin/demo.rs → <crate>.demo


def test_python_and_rust_edges_coexist(tmp_path):
    """混语言：同一根下两边的边都在（Python 路径语义不变）。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text("use vxl_phys_render::Sprite;\n", encoding="utf-8")
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "a.py").write_text("import vxl_phys_render\n", encoding="utf-8")
    rp = _rules(tmp_path, [
        {"id": "r-rs", "from": ["vxl-phys-solver.*"], "import": ["vxl_phys_render.Sprite"]},
        {"id": "r-py", "from": ["pkg.*"], "import": ["vxl_phys_render"]},
    ])
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert got.returncode == 1, got.stdout
    assert "r-rs" in got.stdout and "r-py" in got.stdout, got.stdout
    assert "violations=2" in got.stdout, got.stdout


def test_edges_are_deterministic(tmp_path):
    """同输入两跑逐字相同（候选集合有序、无哈希序渗入）。"""
    src = _ws(tmp_path)
    (src / "sim.rs").write_text(
        "use vxl_phys_render::{Sprite, atlas};\nuse vxl_phys_core::Vec3;\n", encoding="utf-8")
    rp = _rules(tmp_path, [dict(SOLVER_NO_RENDER)])
    a = _run("--root", str(tmp_path), "--rules", str(rp))
    b = _run("--root", str(tmp_path), "--rules", str(rp))
    assert a.stdout == b.stdout and a.returncode == b.returncode, (a.stdout, b.stdout)
