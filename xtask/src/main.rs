//! ADV 质量门载体（xtask bin，逻辑在 lib：
//! `cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants [--base ref] [--update]>`）。
//!
//! 每条门路径都必须以一行 `<门>: 绿|红（N 条）|判不了（原因）` 收尾（见 `xtask::verdict`）——
//! "门没判"和"门判绿"不许在输出上同形。

use std::path::PathBuf;
use xtask::{god, lockstep, mir, mutants, suppress, verdict};

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

/// 三个子门的合档：任一子步抛错 ⇒ 整档判"判不了"，而不是留下一句裸错误。
fn gate_run(root: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let god_violations = god::run(root)?;
    lockstep::run(root)?;
    let sup = suppress::run(root, &adv_core::today())?;
    Ok(verdict::merge_gate(&[
        ("god", god_violations),
        ("suppress", sup),
    ]))
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
            let outcome = gate_run(&root);
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
        Some("mutants") => {
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
            let name = mutants::verdict_name(since.as_deref());
            emit(
                &name,
                mutants::run(&root, &base, since.as_deref(), update, 60),
            );
        }
        Some("mir") => emit("mir", mir::run()),
        _ => {
            eprintln!(
                "用法：cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants>"
            );
            std::process::exit(2);
        }
    }
}
