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
extern crate rustc_session;
extern crate rustc_span;

mod taint_reach;

use adv_ast_rust::spec::{Spec, specs_from_rules};
use adv_ast_rust::wrapper::{crate_output_name, forwarded_args, is_wrapper};
use anyhow::Context;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::ConstContext;
use rustc_hir::def_id::LocalDefId;
use rustc_interface::interface;
use rustc_middle::mir::{self, Operand, TerminatorKind};
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
    /// 直通档的输出文件（`None` = cargo 的目标探测，没有发现可落）。
    wrapper_out: Option<PathBuf>,
    /// 直通档总开关：决定采完后是放 rustc 走完（cargo 要工件）还是即停。
    passthrough: bool,
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
        if let Some(out) = self.wrapper_out.clone() {
            // 落盘放在这里而不是 run_compiler 之后：rustc 的成功路径可能自行结束进程，
            // 放到后面就静默丢发现（丢发现 = 假绿入口）。
            emit_wrapper(self, &out).unwrap_or_else(|e| {
                eprintln!("adv-ast-rust-driver：{e:#}");
                std::process::exit(2);
            });
        }
        // 单文件档采完即停（不留编译输出）；直通档必须走完，cargo 才拿到它要的工件。
        if self.passthrough {
            Compilation::Continue
        } else {
            Compilation::Stop
        }
    }
}

/// 取一个 def 的 MIR body。
///
/// 不能一律走 `optimized_mir`：const 上下文会 panic —— 断言在
/// `rustc_mir_transform/src/lib.rs:790-797`，它按 `hir_body_const_context` 分派，
/// `Some(非 ConstFn)` 直接炸。2026-10-06 深轨第一次编真实 crate（`adv-core`）就炸在
/// `Const { allow_const_fn_promotion: true }` 上。分派规则照 rustc 自己的
/// `instance_mir`（`rustc_middle/src/ty/mod.rs:1955-1977`）：const 项/static/anon const
/// 走 `mir_for_ctfe`，`const fn` 仍可走优化版。
pub(crate) fn mir_body<'tcx>(tcx: TyCtxt<'tcx>, def: LocalDefId) -> &'tcx mir::Body<'tcx> {
    match tcx.hir_body_const_context(def) {
        None | Some(ConstContext::ConstFn) => tcx.optimized_mir(def),
        Some(_) => tcx.mir_for_ctfe(def),
    }
}

/// 取一个函数体内所有 `TerminatorKind::Call` 的调用点。
///
/// 1.99 实况：`Rvalue` 已无 `Call` 变体，调用一律是终结点 ⇒ 只需扫 terminator。
fn call_sites(tcx: TyCtxt<'_>, def: LocalDefId, function: &str) -> Vec<CallSite> {
    let body = mir_body(tcx, def);
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
    /// 直通档：`files` 里装的是 cargo 递给包装器的整串 rustc 参数。
    as_rustc: bool,
}

fn parse_args() -> anyhow::Result<Invocation> {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    let as_rustc = is_wrapper(&argv, std::env::var("ADV_MIR_WRAPPER").ok().as_deref());
    if argv.first().map(String::as_str) == Some("--as-rustc") {
        argv.remove(0);
    }
    if as_rustc {
        anyhow::ensure!(
            argv.len() > 1,
            "直通档要接 cargo 递来的 rustc 参数（rustc 路径 + 至少一个实参）"
        );
        return Ok(Invocation {
            dump_calls: false,
            rules_dir: None,
            files: argv,
            as_rustc: true,
        });
    }
    let mut inv = Invocation {
        dump_calls: false,
        rules_dir: None,
        files: Vec::new(),
        as_rustc: false,
    };
    let mut args = argv.into_iter();
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
        "用法：adv-ast-rust-driver [--dump-calls | --taint <规则目录> | --as-rustc] <文件.rs>…（\
         cargo 包装器档请设 ADV_MIR_WRAPPER=1）"
    );
    Ok(inv)
}

/// 一条命中的 JSON 形态（两档共用：单文件档进 stdout，直通档进 `<crate>.jsonl`）。
fn finding_json(hit: &Hit) -> serde_json::Value {
    serde_json::json!({
        "rule": hit.rule,
        "function": hit.function,
        "location": hit.location,
        "file": hit.file,
        "start_line": hit.span.start_line,
        "start_col": hit.span.start_col,
        "end_line": hit.span.end_line,
        "end_col": hit.span.end_col,
        "engine": "mir",
    })
}

/// 直通档的落盘：按 crate 写 JSONL。stdout 留给 cargo 的诊断通道，不往那儿喷发现。
fn emit_wrapper(probe: &MirProbe, out_file: &Path) -> anyhow::Result<()> {
    let mut text = String::new();
    for hit in &probe.hits {
        text.push_str(&finding_json(hit).to_string());
        text.push('\n');
    }
    std::fs::write(out_file, text)
        .with_context(|| format!("写直通档输出 {}", out_file.display()))?;
    Ok(())
}

fn emit(probe: &MirProbe, rules_dir: Option<&Path>) {
    if rules_dir.is_some() {
        for hit in &probe.hits {
            println!("{}", finding_json(hit));
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
    // 直通档的规则目录与输出目录只认环境变量：深轨不自带规则表，且"静默产空"就是
    // 假绿入口，所以缺任何一个都直接红（不是退回某个默认值继续跑）。
    let (specs, wrapper_out) = if inv.as_rustc {
        let rules = std::env::var("ADV_MIR_RULES")
            .context("直通档（--as-rustc）要环境变量 ADV_MIR_RULES 指定规则目录")?;
        let out_dir = std::env::var("ADV_MIR_OUT")
            .context("直通档（--as-rustc）要环境变量 ADV_MIR_OUT 指定 findings JSONL 的输出目录")?;
        let specs = specs_from_rules(Path::new(&rules))
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("读规则目录 {rules}"))?;
        std::fs::create_dir_all(&out_dir).with_context(|| format!("建直通档输出目录 {out_dir}"))?;
        let name = crate_output_name(&inv.files);
        (
            Some(specs),
            Some(PathBuf::from(out_dir).join(format!("{name}.jsonl"))),
        )
    } else {
        let specs = match &inv.rules_dir {
            Some(dir) => Some(
                specs_from_rules(dir)
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("读规则目录 {}", dir.display()))?,
            ),
            None => None,
        };
        (specs, None)
    };

    // 单文件档：中间产物落独立临时目录，跑完就清。直通档：cargo 的 `--out-dir` 原样保留，
    // 工件必须落回它期望的位置，所以这里不造临时目录也不拼 --crate-type/--edition。
    let (args, tmp_dir) = if inv.as_rustc {
        (forwarded_args(&inv.files, env!("ADV_SYSROOT")), None)
    } else {
        let out_dir = std::env::temp_dir().join(format!("adv-mir-{}", std::process::id()));
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("建临时 out-dir {}", out_dir.display()))?;
        let mut built = vec![
            "adv-ast-rust-driver".to_string(),
            "--edition=2024".to_string(),
            "--crate-type=lib".to_string(),
            format!("--sysroot={}", env!("ADV_SYSROOT")),
            format!("--out-dir={}", out_dir.display()),
        ];
        built.extend(inv.files.clone());
        (built, Some(out_dir))
    };

    let mut probe = MirProbe {
        dump_calls: inv.dump_calls,
        taint_specs: specs,
        wrapper_out,
        passthrough: inv.as_rustc,
        names: Vec::new(),
        calls: Vec::new(),
        hits: Vec::new(),
    };
    rustc_driver::run_compiler(&args, &mut probe);

    // 直通档的发现已在 after_analysis 里落盘（那边之后 rustc 才继续产出工件）。
    if let Some(dir) = tmp_dir {
        emit(&probe, inv.rules_dir.as_deref());
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}
