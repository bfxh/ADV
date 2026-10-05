//! 深轨边车驱动（片A1）：以库方式驱动 rustc，在 `after_analysis` 取 MIR 面，
//! 打印该 crate 的函数名列表（docs/PLAN-deep-track.md §2 输入形态片A、§5 片A1 验收）。
//!
//! 边车不进主进程：本二进制由 xtask/CLI 以子进程调用，rustc_private 的不稳定崩溃
//! 隔离在这里（RESEARCH 01 边车纪律）。
//!
//! 运行前置：本 exe 链接 `rustc_driver` 的 DLL，加载器需要工具链 `bin/` 在 PATH 里；
//! 调用方（xtask / 测试）负责补 PATH，路径由 build.rs 经 `ADV_RUSTC_BIN_DIR` 交出。

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;

use anyhow::Context;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

/// 收集 after_analysis 时刻有 MIR 的函数名（`mir_keys` = 泛型单态化后可出 MIR 的主体）。
struct MirProbe {
    names: Vec<String>,
}

impl Callbacks for MirProbe {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        self.names = tcx
            .mir_keys(())
            .iter()
            .map(|&def_id| tcx.def_path_str(def_id))
            .collect();
        Compilation::Stop
    }
}

fn main() -> anyhow::Result<()> {
    let files: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        !files.is_empty(),
        "用法：adv-ast-rust-driver <文件.rs>… —— 输出该 crate 的 MIR 函数名列表"
    );

    // 中间产物落独立临时目录：片A1 在 after_analysis 即 Stop，不留编译输出。
    let out_dir = std::env::temp_dir().join(format!("adv-mir-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("建临时 out-dir {}", out_dir.display()))?;

    let mut args = vec![
        "adv-ast-rust-driver".to_string(),
        "--edition=2024".to_string(),
        "--crate-type=lib".to_string(),
        format!("--sysroot={}", env!("ADV_SYSROOT")),
        format!("--out-dir={}", out_dir.display()),
    ];
    args.extend(files);

    let mut probe = MirProbe { names: Vec::new() };
    rustc_driver::run_compiler(&args, &mut probe);

    let mut names = probe.names;
    names.sort();
    for name in &names {
        println!("{name}");
    }
    let _ = std::fs::remove_dir_all(&out_dir);
    Ok(())
}
