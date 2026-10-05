//! 深轨边车驱动（片A1 空驱动 + 片A2 污点/取证档）：以库方式驱动 rustc，在
//! `after_analysis` 取 MIR 面（docs/PLAN-deep-track.md §2 输入形态片A、§5）。
//!
//! 用法：
//! - `adv-ast-rust-driver <文件.rs>…` —— 输出该 crate 的 MIR 函数名清单（片A1 验收）
//! - `adv-ast-rust-driver --dump-calls <文件.rs>…` —— 逐调用点输出
//!   `函数\t目标路径\t目的局部\t实参局部`（写规则 YAML 前的取证档：`def_path_str`
//!   的真实形态必须实测，不能按记忆猜）
//! - `adv-ast-rust-driver --taint <规则目录> <文件.rs>…` —— 跑片A2 的污点到达分析，
//!   每行一条 finding（四要素一律从 `rules/**.yaml` 读，深轨不自带规则表）
//!
//! 边车不进主进程：本二进制由 xtask/CLI 以子进程调用，rustc_private 的不稳定崩溃
//! 隔离在这里（RESEARCH 01 边车纪律）。
//!
//! 运行前置：本 exe 链接 `rustc_driver` 的 DLL，加载器需要工具链 `bin/` 在 PATH 里；
//! 调用方（xtask / 测试）负责补 PATH，路径由 build.rs 经 `ADV_RUSTC_BIN_DIR` 交出。

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;

mod taint_reach;

use adv_ast_rust::spec::{Spec, specs_from_rules};
use anyhow::Context;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LocalDefId;
use rustc_interface::interface;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{self, TyCtxt};
use std::path::{Path, PathBuf};
use taint_reach::Hit;

/// 一个函数在 MIR 里可见的调用点（取证用）。
struct CallSite {
    function: String,
    callee: String,
    destination: String,
    args: Vec<String>,
}

/// after_analysis 时收集：函数名清单，按档再加调用点/污点命中。
struct MirProbe {
    dump_calls: bool,
    taint_specs: Option<Vec<Spec>>,
    names: Vec<String>,
    calls: Vec<CallSite>,
    hits: Vec<Hit>,
}

impl MirProbe {
    /// 收集函数名，并按档下钻调用点与污点到达。
    fn harvest(&mut self, tcx: TyCtxt<'_>) {
        let defs: Vec<LocalDefId> = tcx.mir_keys(()).iter().copied().collect();
        self.names = defs.iter().map(|&def| tcx.def_path_str(def)).collect();
        if self.dump_calls {
            for def in defs {
                let function = tcx.def_path_str(def);
                self.calls.extend(call_sites(tcx, def, &function));
            }
        }
        if let Some(specs) = self.taint_specs.clone() {
            self.hits = taint_reach::analyze(tcx, &specs);
        }
    }
}

impl Callbacks for MirProbe {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        self.harvest(tcx);
        Compilation::Stop
    }
}

/// 取一个函数体内所有 `TerminatorKind::Call` 的调用点。
///
/// 1.99 实况：`Rvalue` 已无 `Call` 变体，调用一律是终结点 ⇒ 只需扫 terminator。
fn call_sites(tcx: TyCtxt<'_>, def: LocalDefId, function: &str) -> Vec<CallSite> {
    let body = tcx.optimized_mir(def);
    let mut sites = Vec::new();
    for data in body.basic_blocks.iter() {
        let Some(term) = &data.terminator else {
            continue;
        };
        let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &term.kind
        else {
            continue;
        };
        let Some(callee) = callee_path(tcx, func) else {
            continue;
        };
        sites.push(CallSite {
            function: function.to_string(),
            callee,
            destination: format!("_{}", destination.local.index()),
            args: args.iter().map(|arg| operand_name(&arg.node)).collect(),
        });
    }
    sites
}

/// callee 的用户可见路径：`Operand::Constant` 里的 `ty::FnDef` 取其 DefId 路径。
/// 非 FnDef（函数指针加载等）返回 None，由调用方跳过。
pub(crate) fn callee_path(tcx: TyCtxt<'_>, func: &Operand<'_>) -> Option<String> {
    match func {
        Operand::Constant(konst) => match konst.const_.ty().kind() {
            ty::FnDef(def, _) => Some(tcx.def_path_str(*def)),
            _ => None,
        },
        _ => None,
    }
}

/// 实参操作数的局部名（污点跟踪的对象就是这些局部）。
fn operand_name(op: &Operand<'_>) -> String {
    match op {
        Operand::Copy(place) | Operand::Move(place) => format!("_{}", place.local.index()),
        Operand::Constant(_) => "const".to_string(),
        Operand::RuntimeChecks(_) => "runtime-checks".to_string(),
    }
}

/// 命令行解析结果。
struct Invocation {
    dump_calls: bool,
    rules_dir: Option<PathBuf>,
    files: Vec<String>,
}

fn parse_args() -> anyhow::Result<Invocation> {
    let mut inv = Invocation {
        dump_calls: false,
        rules_dir: None,
        files: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dump-calls" => inv.dump_calls = true,
            "--taint" => {
                let dir = args.next().context("--taint 后面要给规则目录")?;
                inv.rules_dir = Some(PathBuf::from(dir));
            }
            other => inv.files.push(other.to_string()),
        }
    }
    anyhow::ensure!(
        !inv.files.is_empty(),
        "用法：adv-ast-rust-driver [--dump-calls | --taint <规则目录>] <文件.rs>…"
    );
    Ok(inv)
}

fn emit(probe: &MirProbe, rules_dir: Option<&Path>) {
    if rules_dir.is_some() {
        for hit in &probe.hits {
            println!(
                "{}",
                serde_json::json!({
                    "rule": hit.rule,
                    "function": hit.function,
                    "location": hit.location,
                    "engine": "mir",
                })
            );
        }
    } else if probe.dump_calls {
        for site in &probe.calls {
            println!(
                "{}\t{}\t{}\t{}",
                site.function,
                site.callee,
                site.destination,
                site.args.join(",")
            );
        }
    } else {
        let mut names = probe.names.clone();
        names.sort();
        for name in &names {
            println!("{name}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    let inv = parse_args()?;
    let specs = match &inv.rules_dir {
        Some(dir) => Some(
            specs_from_rules(dir)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("读规则目录 {}", dir.display()))?,
        ),
        None => None,
    };

    // 中间产物落独立临时目录：在 after_analysis 即 Stop，不留编译输出。
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
    args.extend(inv.files);

    let mut probe = MirProbe {
        dump_calls: inv.dump_calls,
        taint_specs: specs,
        names: Vec::new(),
        calls: Vec::new(),
        hits: Vec::new(),
    };
    rustc_driver::run_compiler(&args, &mut probe);

    emit(&probe, inv.rules_dir.as_deref());
    let _ = std::fs::remove_dir_all(&out_dir);
    Ok(())
}
