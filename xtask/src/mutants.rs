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
//!
//! 盘量面（片A4 遗留）：cargo-mutants 的 scratch 工作副本开在系统临时目录里，本机
//! TMP 常在 C 盘而 C 盘常年 99% 满。门因此加了两件事——跑之前一次盘量预检（不足直接红，
//! 附带一行修法），跑之后从 unviable 条目的日志里认盘满签名（判红而不是让它伪装成 unviable）。
//! 盘量查不到时如实打"未知"并继续，不拿"查不到"当"没空间"，也不拿它当判据。

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 基线文件位置（相对工作区根）。
pub const BASELINE_PATH: &str = "tools/baselines/mutants-baseline.json";

/// 说明变异真跑到了判定环节的 summary（Unviable = 变异自身没编译/没运行，不算可验证）。
const VERIFIABLE_SUMMARIES: &[&str] = &["CaughtMutant", "MissedMutant", "TimeoutMutant"];

/// 盘量预检下限（GiB），可用 `ADV_MUTANTS_MIN_FREE_GIB` 覆盖。
///
/// 8 不是测出来的门用量，是两个观测点之间的取值：2026-10-05 实测 C 盘剩 4.1GB 时链接期
/// ENOSPC（17 条变异没跑到测试阶段），TMP 挪到 D 盘（剩 38GB）同面跑通。它是体验闸门，
/// 判据不依赖这个数——真盘满由 [`DISK_FAILURE_SIGNATURES`] 负责判红。
const DEFAULT_MIN_FREE_GIB: u64 = 8;

/// 盘满/空间不足在构建日志里的签名（实测原话 + 两侧工具的措辞）。
const DISK_FAILURE_SIGNATURES: &[&str] = &[
    "No space left on device",
    "final link failed",
    "ENOSPC",
    "os error 112",
    "not enough space",
    "磁盘空间不足",
];

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

/// 盘量下限取值：环境变量覆盖，坏值/非正数回退默认（预检不该把自己配崩）。
pub fn min_free_gib(raw: Option<String>) -> u64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MIN_FREE_GIB)
}

/// 预检判红：只有**实测到**余量且低于下限才判红；`None`（查不到）不判红，由调用方如实播报。
pub fn scratch_is_short(free_bytes: Option<u64>, min_gib: u64) -> bool {
    free_bytes.is_some_and(|b| b < min_gib * 1024 * 1024 * 1024)
}

/// 从路径里取盘符（Windows）：`C:\Users\x\AppData\Local\Temp` → `C`。
pub fn drive_letter(path: &Path) -> Option<String> {
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Some(char::from(bytes[0]).to_ascii_uppercase().to_string());
    }
    None
}

/// 字节数读法：`1.9GB` / `未知`。
pub fn human_bytes(free_bytes: Option<u64>) -> String {
    match free_bytes {
        Some(b) => format!("{:.1}GB", b as f64 / (1024f64 * 1024f64 * 1024f64)),
        None => "未知".to_string(),
    }
}

/// 查 `path` 所在盘的可用字节。查不到返回 `None`——不猜数、也不把"查不到"当成"没空间"。
///
/// 全仓 `unsafe_code = "deny"`，所以不直接调 `GetDiskFreeSpaceExW`，改问原生命令：
/// Windows 走 PowerShell 的 `Get-PSDrive`（系统自带，PATH 不依赖 Git Bash），
/// 其余平台走 `df -kP`。
pub fn free_bytes_at(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    if let Some(letter) = drive_letter(path)
        && let Some(bytes) = free_bytes_via_powershell(&letter)
    {
        return Some(bytes);
    }
    free_bytes_via_df(path)
}

/// `Get-PSDrive <D>`.Free（字节）。
#[cfg(windows)]
fn free_bytes_via_powershell(letter: &str) -> Option<u64> {
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("(Get-PSDrive -Name {letter}).Free"),
        ])
        .output()
        .ok()?;
    parse_u64_lines(&out.stdout)
}

/// `df -kP <path>` 的 Available 列（1024 字节块）。
fn free_bytes_via_df(path: &Path) -> Option<u64> {
    let out = std::process::Command::new("df")
        .args(["-kP", path.to_str()?])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // 表头行 + 数据行；数据行第 4 列是 Available（1K 块）。
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let blocks = line.split_whitespace().nth(3)?;
    blocks
        .trim_end_matches('K')
        .parse::<u64>()
        .ok()
        .map(|b| b * 1024)
}

/// 从命令 stdout 里取第一个纯数字行。
fn parse_u64_lines(raw: &[u8]) -> Option<u64> {
    String::from_utf8_lossy(raw)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()))?
        .parse()
        .ok()
}

/// 日志文本里的盘满签名（命中即返回该签名，便于报错时带上原话）。
pub fn disk_failure_signature(log: &str) -> Option<&'static str> {
    DISK_FAILURE_SIGNATURES
        .iter()
        .find(|sig| log.contains(**sig))
        .copied()
}

/// 从 unviable 条目的日志里认盘满：命中的键连同签名与日志路径报出来。
///
/// 这是"环境性假 unviable"的正判据——盘量预检只是提前拦住，真正的结论来自产物日志本身。
/// 日志文件读不到就跳过该条（不猜），但这类情况会被可验证性判据兜住。
pub fn unviable_disk_failures(parsed: &serde_json::Value, out_dir: &Path) -> Vec<String> {
    let Some(outcomes) = parsed["outcomes"].as_array() else {
        return vec![];
    };
    let mut hits = vec![];
    for outcome in outcomes {
        if outcome["summary"].as_str() != Some("Unviable") {
            continue;
        }
        let Some(key) = outcome_key(outcome) else {
            continue;
        };
        let Some(log_name) = outcome["log_path"].as_str() else {
            continue;
        };
        let log = out_dir.join("mutants.out").join(log_name);
        let Ok(text) = std::fs::read_to_string(&log) else {
            continue;
        };
        if let Some(sig) = disk_failure_signature(&text) {
            hits.push(format!(
                "unviable 是盘满造成的缺测（日志含 \"{sig}\"）：{key} · 日志 {}",
                log.display()
            ));
        }
    }
    hits
}

/// 跑之前的盘量预检：余量不足则红，并给一行可直接照抄的修法。
fn precheck_scratch(min_gib: u64) -> Vec<String> {
    let scratch = std::env::temp_dir();
    let free = free_bytes_at(&scratch);
    println!(
        "预检 TMP={} 余量={} 下限={min_gib}GB",
        scratch.display(),
        human_bytes(free)
    );
    if scratch_is_short(free, min_gib) {
        let bytes = free.unwrap_or_default();
        return vec![format!(
            "TMP 所在盘余量不足（{:.1}GB < {min_gib}GB）：cargo-mutants 会在这里开 scratch，盘满时链接失败只判 unviable，门会缺测。一行修法：把 TMP 指到宽裕的盘再跑，例如 TMP=D:/tmp/adv-mut TEMP=D:/tmp/adv-mut cargo run -p xtask -- mutants",
            bytes as f64 / (1024f64 * 1024f64 * 1024f64)
        )];
    }
    vec![]
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
    let min_gib = min_free_gib(std::env::var("ADV_MUTANTS_MIN_FREE_GIB").ok());
    let short = precheck_scratch(min_gib);
    if !short.is_empty() {
        return Ok(short);
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
    let [total, caught, missed_n, unviable_n] = tally(&parsed);
    println!("变异面 总={total} 捕获={caught} 未捕获={missed_n} unviable={unviable_n}");
    if unviable_n > 0 {
        println!(
            "提示：unviable {unviable_n} 条的日志在 {}/mutants.out/log/；条数增多先怀疑 scratch 盘满/链接失败，别把缺测读成没问题",
            out_dir.display()
        );
    }
    let disk = unviable_disk_failures(&parsed, &out_dir);
    let baseline_path = root.join(BASELINE_PATH);
    if update {
        if !disk.is_empty() {
            println!("基线未改动：本轮有盘满缺测，重录会把债当成已清带下去");
            return Ok(disk);
        }
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
    violations.extend(disk);
    Ok(violations)
}

/// 本轮变异面计数：总/捕获/未捕获/unviable。
///
/// 打印用（把"unviable 增多"摆到明面上），不参与判红；判盘满缺测的是
/// [`unviable_disk_failures`]。`outcomes` 畸形时返回全 0——真正的畸形由 `keys_by_summary`
/// 先判红，这里不重复定罪。
pub fn tally(parsed: &serde_json::Value) -> [usize; 4] {
    let outcomes: &[serde_json::Value] = parsed["outcomes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let count = |want: &str| {
        outcomes
            .iter()
            .filter(|o| o["summary"].as_str() == Some(want))
            .count()
    };
    [
        outcomes.len(),
        count("CaughtMutant"),
        count("MissedMutant"),
        count("Unviable"),
    ]
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
