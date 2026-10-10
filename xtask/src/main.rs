//! ADV 质量门载体（xtask bin，逻辑在 lib：
//! `cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants [--base ref] [--update]
//! | mutants --rotate [轮转窗口档]>`）。
//!
//! 每条门路径都必须以一行 `<门>: 绿|红（N 条）|判不了（原因）` 收尾（见 `xtask::verdict`）——
//! "门没判"和"门判绿"不许在输出上同形。

use std::path::PathBuf;
use xtask::{gate, god, lockstep, mir, mutants, rotation, suppress, verdict};

fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("xtask 不在工作区根下"))
}

/// 出裁决并退出。红 = 1，判不了 = 3（与红可区分，包装器能分别处置）。
fn emit(name: &str, outcome: anyhow::Result<Vec<String>>) -> ! {
    let (line, code) = verdict::verdict(name, &outcome);
    if code == 0 {
        println!("{line}");
    } else {
        eprintln!("{line}");
    }
    std::process::exit(code);
}

fn main() {
    let root = match workspace_root() {
        Ok(r) => r,
        Err(e) => emit("xtask", Err(e)),
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("god") => {
            if args.contains(&"--write".to_string()) {
                // 重录是"登记"动作不是判定：它自己打印逐键点名，退出码 0/1 由 init_or_write 决定。
                if let Err(e) = god::init_or_write(&root) {
                    emit("god", Err(e));
                }
                return;
            }
            emit("god", god::run(&root));
        }
        Some("lockstep") => {
            if let Err(e) = lockstep::run(&root) {
                emit("lockstep", Err(e));
            }
            println!("lockstep: 绿");
        }
        Some("gate") => {
            let outcome = gate::run(&root);
            if outcome.as_ref().is_ok_and(|v| v.is_empty()) {
                // 绿时才补打这两行——保持既有形状（红/判不了由 emit 单点输出）。
                println!("gate: 绿");
                println!("lockstep: 绿");
                println!("suppress: 绿");
                return;
            }
            emit("gate", outcome);
        }
        Some("suppress") => emit("suppress", suppress::run(&root, &adv_core::today())),
        Some("mutants") => mutants_gate(&root, &args),
        Some("mir") => emit("mir", mir::run()),
        _ => {
            eprintln!(
                "用法：cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants \
                 [--base ref] [--since rev] [--update] | mutants --rotate>"
            );
            std::process::exit(2);
        }
    }
}

/// `mutants` 子命令的三档分派：全档 / `--since` 增量 / `--rotate` 轮转窗口。
///
/// 搬出 `main` 是因为 `main` 在这个仓里是**分发壳**（god 尺管着它的行数），加一档就顶一次阈；
/// 分档口径本身（谁能配什么、拒绝什么）由 `mutants`/`rotation` 两个模块各自说清。
fn mutants_gate(root: &std::path::Path, args: &[String]) -> ! {
    // scratch 落点与 timeout 两个旋钮在分派最早段收进来：门自己指盘、自己定超时，
    // 不指望调用方环境还记得带（实测后台链丢过 TMP）。
    if let Err(e) = mutants::ensure_scratch() {
        eprintln!("{e:#}");
        std::process::exit(2);
    }
    let (timeout, warn) =
        mutants::timeout_secs(std::env::var("ADV_MUTANTS_TIMEOUT").ok().as_deref());
    if let Some(note) = warn {
        eprintln!("{note}");
    }
    let base = args
        .windows(2)
        .find(|w| w[0] == "--base")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "main".to_string());
    let update = args.contains(&"--update".to_string());
    let since = args
        .windows(2)
        .find(|w| w[0] == "--since")
        .map(|w| w[1].clone());
    if args.contains(&"--rotate".to_string()) {
        let (budget, warn) =
            rotation::window_budget(std::env::var("ADV_MUTANTS_WINDOW_BUDGET").ok().as_deref());
        if let Some(note) = warn {
            eprintln!("{note}");
        }
        emit(
            &rotation::verdict_name(budget),
            rotation::run(root, &base, budget, update, timeout),
        );
    }
    emit(
        &mutants::verdict_name(since.as_deref()),
        mutants::run(root, &base, since.as_deref(), update, timeout),
    )
}
