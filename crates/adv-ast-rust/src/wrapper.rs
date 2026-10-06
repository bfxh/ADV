//! cargo 包装器档的三个纯判据：怎么进直通档、参数怎么转、输出文件叫什么名。
//!
//! 放在 lib 侧（不碰 `rustc_private`）的用法是让**同包测试**能直接喂数据杀变异——
//! 这三处都是"分支观察面"的高发地，片A6 刚因同类问题被门虚计过一次。

/// 直通档的触发条件（两条路都认）：
/// - `ADV_MIR_WRAPPER=1` —— cargo 的 `RUSTC_WORKSPACE_WRAPPER` 形态。cargo 只会递
///   `<包装器> <rustc 路径> <rustc 参数…>`，**没有插自定义 flag 的位置**（2026-10-06 实测：
///   自定义 `--as-rustc` 在第一次目标探测 `rustc -vV` 时就被当成输入文件，
///   ⇒ `error: multiple input filenames provided`）。
/// - `--as-rustc` 作 argv[0] —— 手测与单测用的显式形态。
pub fn is_wrapper(argv: &[String], env_wrapper: Option<&str>) -> bool {
    argv.first().map(String::as_str) == Some("--as-rustc") || env_wrapper == Some("1")
}

/// 转发给 `run_compiler` 的参数：原样保留 cargo 递来的串，只在 args[0] 之后补 sysroot。
///
/// 必须补：rustc 缺省按**自身可执行文件位置**推 sysroot，而驱动 exe 在 `target/debug`，
/// 那儿没有 `lib/rustlib/<host>/lib` ⇒ 找不到 `core`。cargo 不传 `--sysroot`，
/// 而 `run_compiler` 会丢掉 args[0]（`rustc_driver_impl/src/lib.rs:183`），所以注入位是 1。
pub fn forwarded_args(files: &[String], sysroot: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(files.len() + 1);
    args.extend(files.iter().take(1).cloned());
    args.push(format!("--sysroot={sysroot}"));
    args.extend_from_slice(&files[1..]);
    args
}

/// 输出文件名（按 crate 分）：取 `--crate-name`，两种写法都认。
///
/// 缺 `--crate-name` 时回退 `unnamed` 而不是"没有输出文件"——静默丢发现就是假绿入口。
/// cargo 的目标探测（`-vV` / `--print=…`）走 `handle_options` 短路，到不了
/// `after_analysis`，所以不会为此留下空文件。
pub fn crate_output_name(files: &[String]) -> String {
    let mut prev_is_name = false;
    for arg in files.iter().skip(1) {
        if let Some(rest) = arg.strip_prefix("--crate-name=") {
            return rest.to_string();
        }
        if prev_is_name {
            return arg.clone();
        }
        prev_is_name = arg == "--crate-name";
    }
    "unnamed".to_string()
}
