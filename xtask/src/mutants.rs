//! 变异门（M2 片3 装配；RESEARCH 03 裁定：只留 `--in-diff` 档进 PR 门，分钟级）。
//!
//! 语义：以 `--base`（默认 main）的 merge-base diff 限定变异面，跑 cargo-mutants，
//! **未捕获（missed）的变异与基线比对——新增 missed 即红**；存量 missed 是已登记
//! 的债（棘轮只准减：修掉一个就从基线划掉一个，`--update` 有意识重录）。
//! cargo-mutants 缺席/超时/输出畸形一律红（fail-closed，不静默降级）。
//!
//! 第二道判据（片A3 加，起因是实测 false green）：**基线里的键本轮必须"可验证"**——
//! 即该键至少有 1 个变异跑到判定环节（caught / missed / timeout）。若全部判 Unviable
//! （或压根不在本轮差异面上），棘轮只看"新增 missed"会照样报绿，债就静默蒸发。
//! 实测的第一案就是环境性假 unviable：`ld.exe: final link failed: No space left on device`
//! 让 `xtask/src/mir.rs`、`mutants.rs` 的 17 条变异没跑到测试阶段（见
//! eval/FP-LEDGER-m2s4.md）。
//!
//! 口径注意：cargo-mutants 默认只跑**变异所在包**的测试，所以跨包的断言对本门不算覆盖；
//! 判据要与被变异代码同包，否则门会误报存活。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 基线文件位置（相对工作区根）。
pub const BASELINE_PATH: &str = "tools/baselines/mutants-baseline.json";

/// 说明变异真跑到了判定环节的 summary（Unviable = 变异自身没编译/没运行，不算可验证）。
const VERIFIABLE_SUMMARIES: &[&str] = &["CaughtMutant", "MissedMutant", "TimeoutMutant"];

#[derive(Deserialize)]
struct Baseline {
    missed: Vec<String>,
}

/// 变异键：`file::function`（从 outcomes 条目提取，cargo-mutants 27 格式）。
pub fn outcome_key(outcome: &serde_json::Value) -> Option<String> {
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

/// 可验证性比较：baseline 里有而本轮"可判定键集"里没有 = 该键本轮不可验证 = 红。
pub fn unverifiable_keys(baseline: &[String], verifiable: &[String]) -> Vec<String> {
    baseline
        .iter()
        .filter(|b| !verifiable.contains(b))
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
    // cargo-mutants 退出码契约：0 = 全捕获；2 = 存在未捕获变异（正文在 stdout）。
    // 两者都继续走棘轮比对；其余退出码才是真失败（fail-closed）。
    anyhow::ensure!(
        matches!(out.status.code(), Some(0) | Some(2)),
        "cargo mutants 失败（exit {:?}）：{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr)
    );
    // cargo-mutants 把结果写在 <out>/mutants.out/outcomes.json
    let outcomes_path = out_dir.join("mutants.out").join("outcomes.json");
    let raw = std::fs::read_to_string(&outcomes_path)
        .with_context(|| format!("读 {}", outcomes_path.display()))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("解析 {}", outcomes_path.display()))?;
    let missed = keys_by_summary(&parsed, |s| s == "MissedMutant")?;
    let verifiable = keys_by_summary(&parsed, |s| VERIFIABLE_SUMMARIES.contains(&s))?;
    let baseline_path = root.join(BASELINE_PATH);
    if update {
        let old = read_baseline(&baseline_path).unwrap_or_else(|_| Baseline { missed: vec![] });
        std::fs::create_dir_all(baseline_path.parent().expect("parent"))?;
        std::fs::write(
            &baseline_path,
            serde_json::to_string_pretty(&serde_json::json!({ "missed": missed }))?,
        )?;
        println!("基线已写 {}（missed {} 条）", BASELINE_PATH, missed.len());
        // 重录是整表替换：本轮没再产的旧键会离开债账，这里点名而不静默。
        let dropped = new_missed(&old.missed, &missed);
        if !dropped.is_empty() {
            eprintln!(
                "注意：本次重录从基线移除了 {} 个键（要么真被杀掉了，要么本轮不可验证）：",
                dropped.len()
            );
            for k in dropped {
                eprintln!("  - {k}");
            }
        }
        return Ok(vec![]);
    }
    let baseline = read_baseline(&baseline_path)?;
    let mut violations: Vec<String> = new_missed(&missed, &baseline.missed)
        .into_iter()
        .map(|k| format!("新增未捕获变异：{k}"))
        .collect();
    violations.extend(
        unverifiable_keys(&baseline.missed, &verifiable)
            .into_iter()
            .map(|k| {
                format!(
                    "基线键本轮无可判定变异（不可验证；多为变异全判 unviable 或构建/链接失败）：{k}"
                )
            }),
    );
    Ok(violations)
}

/// 按 summary 谓词从 outcomes 提取键（排序去重）。
pub fn keys_by_summary(
    parsed: &serde_json::Value,
    take: impl Fn(&str) -> bool,
) -> Result<Vec<String>> {
    let mut keys: Vec<String> = parsed["outcomes"]
        .as_array()
        .context("outcomes 应为数组")?
        .iter()
        .filter(|o| o["summary"].as_str().is_some_and(&take))
        .filter_map(outcome_key)
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// 读基线（缺失即红——不允许"没有基线"当成"没有债"）。
fn read_baseline(path: &Path) -> Result<Baseline> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("读 {}", path.display()))?;
    Ok(serde_json::from_str(&raw)?)
}
