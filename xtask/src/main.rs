//! ADV 质量门载体（xtask bin，逻辑在 lib：
//! `cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants [--base ref] [--update]>`）。

use std::path::PathBuf;
use xtask::{god, lockstep, mir, mutants, suppress};

fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("xtask 不在工作区根下"))
}

fn main() -> anyhow::Result<()> {
    let root = workspace_root()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("god") => {
            if args.contains(&"--write".to_string()) {
                god::init_or_write(&root)?;
                return Ok(());
            }
            let violations = god::run(&root)?;
            report("god", violations);
        }
        Some("lockstep") => {
            lockstep::run(&root)?;
            println!("lockstep: 绿");
        }
        Some("gate") => {
            let mut violations = god::run(&root)?;
            lockstep::run(&root)?;
            violations.extend(suppress::run(&root, &adv_core::today())?);
            report("gate", violations);
            println!("lockstep: 绿");
            println!("suppress: 绿");
        }
        Some("suppress") => {
            let violations = suppress::run(&root, &adv_core::today())?;
            report("suppress", violations);
        }
        Some("mutants") => {
            let base = args
                .windows(2)
                .find(|w| w[0] == "--base")
                .map(|w| w[1].clone())
                .unwrap_or_else(|| "main".to_string());
            let update = args.contains(&"--update".to_string());
            let violations = mutants::run(&root, &base, update, 60)?;
            report("mutants", violations);
        }
        Some("mir") => report("mir", mir::run()?),
        _ => {
            eprintln!(
                "用法：cargo run -p xtask -- <god [--write] | lockstep | gate | mir | mutants>"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

fn report(name: &str, violations: Vec<String>) {
    if violations.is_empty() {
        println!("{name}: 绿");
        return;
    }
    eprintln!("{name}: 红（{} 条）", violations.len());
    for v in &violations {
        eprintln!("  - {v}");
    }
    std::process::exit(1);
}
