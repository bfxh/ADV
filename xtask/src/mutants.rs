//! 变异门（M2 片3 装配；RESEARCH 03 裁定：只留 `--in-diff` 档进 PR 门，分钟级）。
//!
//! 语义：以 `--base`（默认 main）的 merge-base diff 限定变异面，跑 cargo-mutants，
//! **未捕获（missed）的变异与基线比对——新增 missed 即红**；存量 missed 是已登记
//! 的债（棘轮只准减：修掉一个就从基线划掉一个，`--update` 有意识重录）。
//! cargo-mutants 缺席/超时/输出畸形一律红（fail-closed，不静默降级）。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 基线文件位置（相对工作区根）。
pub const BASELINE_PATH: &str = "tools/baselines/mutants-baseline.json";

#[derive(Deserialize)]
struct Baseline {
    missed: Vec<String>,
}

/// 未捕获变异键：`file::function`（从 outcomes 条目提取，cargo-mutants 27 格式）。
fn missed_key(outcome: &serde_json::Value) -> Option<String> {
    let file = outcome
        .pointer("/scenario/Mutant/file")?
        .as_str()?
        .to_string();
    let function = outcome
        .pointer("/scenario/Mutant/function/function_name")?
        .as_str()?
        .to_string();
    Some(format!("{file}::{function}"))
}

/// 棘轮比较：current 里有而 baseline 没有的 missed = 新债 = 红。
pub fn new_missed(current: &[String], baseline: &[String]) -> Vec<String> {
    current
        .iter()
        .filter(|c| !baseline.contains(c))
        .cloned()
        .collect()
}

fn git_diff_patch(root: &Path, base: &str) -> Result<PathBuf> {
    let patch = root.join("target/mutants-diff.patch");
    let out = std::process::Command::new("git")
        .args(["diff", "--binary", &format!("{base}...HEAD")])
        .current_dir(root)
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "git diff 失败：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::create_dir_all(patch.parent().expect("parent"))?;
    std::fs::write(&patch, &out.stdout)?;
    Ok(patch)
}

/// 跑变异门。`base` 为 diff 基线 ref；`update` 重录基线（披露通道）。
pub fn run(root: &Path, base: &str, update: bool, timeout_secs: u64) -> Result<Vec<String>> {
    let patch = git_diff_patch(root, base)?;
    let patch_text = std::fs::read_to_string(&patch)?;
    if patch_text.trim().is_empty() {
        return Ok(vec![format!(
            "skip: 与 {base} 无差异，无变异面（skip 不算绿——门在此档视为通过并留痕）"
        )]);
    }
    let out_dir = root.join("target/mutants-out");
    let _ = std::fs::remove_dir_all(&out_dir);
    let out = std::process::Command::new("cargo")
        .args([
            "mutants",
            "--in-diff",
            patch.to_str().context("patch 路径")?,
            "-o",
            out_dir.to_str().context("out 路径")?,
            "--no-shuffle",
            "-j",
            "4",
            "--timeout",
            &timeout_secs.to_string(),
        ])
        .current_dir(root)
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "cargo mutants 失败：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // cargo-mutants 把结果写在 <out>/mutants.out/outcomes.json
    let outcomes_path = out_dir.join("mutants.out").join("outcomes.json");
    let raw = std::fs::read_to_string(&outcomes_path)
        .with_context(|| format!("读 {}", outcomes_path.display()))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("解析 {}", outcomes_path.display()))?;
    let mut missed: Vec<String> = parsed["outcomes"]
        .as_array()
        .context("outcomes 应为数组")?
        .iter()
        .filter(|o| o["summary"] == "MissedMutant")
        .filter_map(missed_key)
        .collect();
    missed.sort();
    missed.dedup();
    if update {
        let baseline_path = root.join(BASELINE_PATH);
        std::fs::create_dir_all(baseline_path.parent().expect("parent"))?;
        std::fs::write(
            &baseline_path,
            serde_json::to_string_pretty(&serde_json::json!({ "missed": missed }))?,
        )?;
        println!("基线已写 {}（missed {} 条）", BASELINE_PATH, missed.len());
        return Ok(vec![]);
    }
    let baseline_path = root.join(BASELINE_PATH);
    let baseline: Baseline = serde_json::from_str(
        &std::fs::read_to_string(&baseline_path)
            .with_context(|| format!("读 {}", baseline_path.display()))?,
    )?;
    Ok(new_missed(&missed, &baseline.missed))
}
