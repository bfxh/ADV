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
import re
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


def test_bom_prefixed_file_is_not_blind(tmp_path):
    """带 UTF-8 BOM 的源文件**不许掉出依赖图**（本仓实测盲过：tests/test_ide_diag.py
    的 BOM 让 ast.parse 撞在 U+FEFF 上 ⇒ 那个文件的 import 边全在门的视野外，
    摘要只留一个 unparsed=1 的计数）。判据两头：违规要能红，且 unparsed 必须归零。"""
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "a.py").write_text("import vxl_phys_render\n", encoding="utf-8-sig")  # 真 BOM 头
    rp = _rules(tmp_path, [{"id": "r-bom", "from": ["pkg", "pkg.*"],
                            "import": ["vxl_phys_render"]}])
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert "unparsed=0" in got.stdout, f"BOM 文件仍被当成不可解析：{got.stdout}"
    assert got.returncode == 1, f"BOM 文件里的违规没被抓到：{got.stdout}"
    assert "r-bom" in got.stdout, got.stdout


def test_every_workspace_crate_is_covered_by_a_rule():
    """**在册 crate 必须都出现在某条规则的 from 里**（钉本仓真实分层，不是夹具）。
    新加一个 crate 而忘给它写禁向规则 ⇒ 这条红；否则分层图会悄悄留出一个无人看的洞
    （2026-10-12 实测：spec/arch-rules.json 原本 4 条规则全是 python 侧，13 个 rust
    crate 一条都没声明，adv-parse 反手 import adv-rules 也不会判红）。"""
    members = sorted(list((ROOT / "crates").glob("*/Cargo.toml")) + [ROOT / "xtask" / "Cargo.toml"])
    crates = []
    for mf in members:
        m = re.search(r'^\s*name\s*=\s*"([^"]+)"', mf.read_text(encoding="utf-8"), re.MULTILINE)
        assert m, f"{mf} 读不出 crate 名"
        crates.append(m.group(1))
    assert len(crates) >= 13, f"crate 清单缩水：{crates}"

    rules = json.loads((ROOT / "spec" / "arch-rules.json").read_text(encoding="utf-8"))["rules"]
    # from 侧只认**连字符形态**（实测：写成 adv_rules 一条都命中不了），`.*` 是子模块通配
    covered = {p[:-2] if p.endswith(".*") else p for r in rules for p in r.get("from", [])}
    missing = [c for c in crates if c not in covered]
    assert not missing, f"这些 crate 没有被任何架构规则覆盖（分层图上无人看的洞）：{missing}"


# ---- uiscan（UI 文案采集器）并入前的分层规则金丝雀（2026-10-16） ----------------
# 规则先于 crate 落地：`from` 只按**扫描到的** crate 匹配，所以规则先写不会误红；
# 但必须先证"它能红"，否则搬完 crate 只是多了一张装饰表。

UISCAN_IDS = {
    "rust-uiscan-core-is-base",
    "rust-uiscan-tui-only-core",
    "rust-uiscan-web-only-core",
    "rust-uiscan-gui-only-core",
    "rust-uiscan-cli-not-into-adv",
    "rust-adv-no-uiscan-dep",
}


def _uiscan_ws(tmp_path: pathlib.Path) -> dict[str, pathlib.Path]:
    """伪造一个含五个 uiscan crate + adv-core 的工作区，返回各 crate 的 src 目录。"""
    srcs = {}
    for c in ("uiscan-core", "uiscan-tui", "uiscan-web", "uiscan-gui", "uiscan-cli", "adv-core"):
        d = tmp_path / "crates" / c
        (d / "src").mkdir(parents=True)
        (d / "Cargo.toml").write_text(
            f'[package]\nname = "{c}"\nversion = "0.1.0"\n', encoding="utf-8")
        (d / "src" / "lib.rs").write_text("pub struct X;\n", encoding="utf-8")
        srcs[c] = d / "src"
    (tmp_path / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\n', encoding="utf-8")
    return srcs


def _uiscan_rules(tmp_path: pathlib.Path) -> pathlib.Path:
    """取**真规则文件**里的 uiscan 六条（不复制一份影子规则）。"""
    real = json.loads((ROOT / "spec" / "arch-rules.json").read_text(encoding="utf-8"))["rules"]
    picked = [r for r in real if r["id"] in UISCAN_IDS]
    assert len(picked) == len(UISCAN_IDS), f"真规则里缺 uiscan 规则：{[r['id'] for r in real]}"
    return _rules(tmp_path, picked)


def test_uiscan_layering_is_quiet_when_dependencies_point_downward(tmp_path):
    """金丝雀静：core←三适配器←cli、adv-core 不碰 uiscan ⇒ 0 违例。"""
    srcs = _uiscan_ws(tmp_path)
    (srcs["uiscan-tui"] / "lib.rs").write_text("use uiscan_core::X;\n", encoding="utf-8")
    (srcs["uiscan-cli"] / "lib.rs").write_text(
        "use uiscan_core::X;\nuse uiscan_tui::X as T;\nuse uiscan_web::X as W;\nuse uiscan_gui::X as G;\n",
        encoding="utf-8")
    rp = _uiscan_rules(tmp_path)
    got = _run("--root", str(tmp_path), "--rules", str(rp))
    assert got.returncode == 0 and "violations=0" in got.stdout, got.stdout


def test_uiscan_rules_go_red_on_all_four_forbidden_edges(tmp_path):
    """每条禁向都要能红，并且定位到 use 行。"""
    cases = [
        ("uiscan-tui", "use uiscan_web::HARVEST_JS;\n", ":1", "rust-uiscan-tui-only-core"),
        ("uiscan-core", "use uiscan_cli::main as m;\n", ":1", "rust-uiscan-core-is-base"),
        ("uiscan-gui", "use uiscan_tui::harvest;\n", ":1", "rust-uiscan-gui-only-core"),
        ("uiscan-cli", "use adv_rules::check;\n", ":1", "rust-uiscan-cli-not-into-adv"),
        ("adv-core", "use uiscan_core::Item;\n", ":1", "rust-adv-no-uiscan-dep"),
    ]
    srcs = _uiscan_ws(tmp_path)
    rp = _uiscan_rules(tmp_path)
    for crate, line, loc, rid in cases:
        f = srcs[crate] / "lib.rs"
        original = f.read_text(encoding="utf-8")
        f.write_text(line + original, encoding="utf-8")
        got = _run("--root", str(tmp_path), "--rules", str(rp))
        f.write_text(original, encoding="utf-8")
        assert got.returncode == 1, f"{crate} 应判红：{got.stdout}"
        assert f"{crate}{loc}" in got.stdout, f"{crate} 定位不对：{got.stdout}"  # lib.rs 折叠成 crate 名
        assert rid in got.stdout, f"{crate} 点错规则：{got.stdout}"
