//! 片B1 验收：cargo 包装器直通档——① 发现按 crate 落 JSONL 且**自带文件名**
//! （整 crate 一档没有"调用方已知文件"这个前提）；② 工件照常产出（`Compilation::Continue`
//! 的证据，cargo 要的就是这个）；③ 缺规则/输出目录环境变量即红，不静默产空。
//! 判据走真路径——起驱动子进程，不在测试里重实现转发逻辑。

use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 起驱动，并把工具链 `bin/` 前置到 PATH（运行期要在那儿找 `rustc_driver` 的 DLL）。
fn driver() -> Command {
    let mut path = OsString::from(env!("ADV_RUSTC_BIN_DIR"));
    path.push(if cfg!(windows) { ";" } else { ":" });
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_adv-ast-rust-driver"));
    cmd.env("PATH", path);
    cmd
}

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate 在工作区根的 crates/ 下")
        .join("rules")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// 每次运行独占的临时根（pid + tag），跑完连根一起清。
fn case_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("adv-wrap-{}-{tag}", std::process::id()))
}

/// cargo 递给包装器的 argv 形态：argv[0] 是 rustc 路径（`run_compiler` 会丢掉它），
/// 其余是它给这个 crate 的真实参数。
fn rustc_argv(rustc: &str, out_dir: &Path, crate_name: &str, input: &Path) -> Vec<String> {
    vec![
        rustc.to_string(),
        format!("--crate-name={crate_name}"),
        "--edition=2024".to_string(),
        "--crate-type=lib".to_string(),
        "--emit=metadata".to_string(),
        format!("--out-dir={}", out_dir.display()),
        input.display().to_string(),
    ]
}

/// 夹具里 `pub fn` 之后首次出现 `needle` 的行号（期望值锚到夹具文本，不手打）。
fn line_of(path: &Path, needle: &str) -> usize {
    let text = std::fs::read_to_string(path).expect("读夹具");
    text.lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .map(|(i, _)| i + 1)
        .unwrap_or_else(|| panic!("{} 里找不到 {needle}", path.display()))
}

#[test]
fn wrapper_mode_writes_findings_with_file_and_still_emits_artifact() {
    let root = case_root("hit");
    let build = root.join("build");
    let mir = root.join("mir");
    std::fs::create_dir_all(&build).expect("建 --out-dir");
    std::fs::create_dir_all(&mir).expect("建 ADV_MIR_OUT");

    let fx = fixture("taint_direct.rs");
    let expect_line = line_of(&fx, "Command::new");
    let mut cmd = driver();
    let out = cmd
        .arg("--as-rustc")
        .args(rustc_argv("rustc", &build, "probe", &fx))
        .env("ADV_MIR_RULES", rules_dir())
        .env("ADV_MIR_OUT", &mir)
        .output()
        .expect("启动 adv-ast-rust-driver 失败");

    // 先把要看的东西读进内存再清临时目录：断言失败也不在盘上留残渣。
    let artifacts: Vec<String> = std::fs::read_dir(&build)
        .expect("读 --out-dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let jsonl_path = mir.join("probe.jsonl");
    let jsonl = std::fs::read_to_string(&jsonl_path)
        .unwrap_or_else(|e| format!("<读 {} 失败：{e}>", jsonl_path.display()));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let ok = out.status.success();
    let _ = std::fs::remove_dir_all(&root);

    assert!(ok, "直通档退出码非零：{stderr}");
    // ① 工件照常产出：只传 --emit=metadata，所以产物是 `*probe.rmeta`
    assert!(
        artifacts.iter().any(|n| n.ends_with("probe.rmeta")),
        "直通档没产出 cargo 要的元数据工件（`after_analysis` 没放行），--out-dir 里是 {artifacts:?}"
    );

    // ② 发现按 crate 落盘，且带真实文件名与行号
    let items: Vec<Value> = jsonl
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("直通档输出应为 JSONL"))
        .collect();
    assert!(
        !items.is_empty(),
        "直通档在已知污点流的夹具上出了 0 条，输出原文：{jsonl}"
    );
    let hit = &items[0];
    assert_eq!(hit["engine"], serde_json::json!("mir"));
    assert_eq!(hit["rule"], serde_json::json!("RS-TAINT-COMMAND"));
    assert!(
        hit["file"]
            .as_str()
            .is_some_and(|f| f.ends_with("taint_direct.rs")),
        "直通档的发现必须自带文件名（整 crate 一档没有调用方已知文件这个前提）：{hit}"
    );
    assert_eq!(
        hit["start_line"].as_u64().map(|v| v as usize),
        Some(expect_line),
        "汇点行没锚回夹具文本"
    );
}

#[test]
fn wrapper_mode_without_rules_or_out_dirs_is_red() {
    // 缺环境变量不能"照常编译但什么都不产出"——静默产空就是假绿入口。
    for (tag, with_rules, with_out) in [("norules", false, true), ("nooutdir", true, false)] {
        let root = case_root(tag);
        let build = root.join("build");
        std::fs::create_dir_all(&build).expect("建 --out-dir");
        let mut cmd = driver();
        cmd.arg("--as-rustc")
            .args(rustc_argv(
                "rustc",
                &build,
                "probe",
                &fixture("taint_direct.rs"),
            ))
            .env_remove("ADV_MIR_RULES")
            .env_remove("ADV_MIR_OUT");
        if with_rules {
            cmd.env("ADV_MIR_RULES", rules_dir());
        }
        if with_out {
            cmd.env("ADV_MIR_OUT", root.join("mir"));
        }
        let out = cmd.output().expect("启动驱动失败");
        let ok = out.status.success();
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        let _ = std::fs::remove_dir_all(&root);
        assert!(!ok, "{tag}：缺环境变量却判成通过");
        assert!(
            err.contains("ADV_MIR_RULES") || err.contains("ADV_MIR_OUT"),
            "{tag}：报错要点名缺哪个环境变量，实得 {err}"
        );
    }
}

/// 三个纯判据的直接喂数据（同包断言，防片A6 那类"分支观察不到就被记成已杀"）。
#[test]
fn wrapper_trigger_forward_and_naming_are_pinned() {
    use adv_ast_rust::wrapper::{crate_output_name, forwarded_args, is_wrapper};

    let argv = |s: &[&str]| -> Vec<String> { s.iter().map(|x| x.to_string()).collect() };
    assert!(is_wrapper(&argv(&["--as-rustc", "rustc"]), None));
    assert!(
        is_wrapper(&argv(&["rustc", "-vV"]), Some("1")),
        "cargo 包装器档只给环境变量这一条路，必须认"
    );
    assert!(!is_wrapper(&argv(&["rustc", "-vV"]), Some("0")));
    assert!(!is_wrapper(&argv(&["file.rs"]), None));

    // sysroot 注入位是 1（args[0] 被 run_compiler 丢掉）
    let got = forwarded_args(&argv(&["rustc.exe", "--crate-name=x", "src/lib.rs"]), "SR");
    assert_eq!(
        got,
        argv(&["rustc.exe", "--sysroot=SR", "--crate-name=x", "src/lib.rs"])
    );

    assert_eq!(
        crate_output_name(&argv(&["rustc", "--crate-name=adv_core", "src/lib.rs"])),
        "adv_core"
    );
    assert_eq!(
        crate_output_name(&argv(&["rustc", "--crate-name", "adv_taint", "src/lib.rs"])),
        "adv_taint"
    );
    // 探测调用（`-vV`）没有 crate 名：回退 unnamed 而不是"没有输出"
    assert_eq!(crate_output_name(&argv(&["rustc", "-vV"])), "unnamed");
    // 两个 crate 的输出名不同 ⇒ 不会互相覆盖
    assert_ne!(
        crate_output_name(&argv(&["r", "--crate-name=a", "x.rs"])),
        crate_output_name(&argv(&["r", "--crate-name=b", "x.rs"]))
    );
}
