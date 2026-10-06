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
/// 可判定的结果标签。字面量锚到 cargo-mutants 27.1.0 真产物（2026-10-06 一轮 376 条
/// 实测标签：`Success` / `Unviable` / `CaughtMutant` / `MissedMutant` / `Timeout`）——
/// 写错一个标签名不会报错，只会让那一类变异从"可验证"里静默消失。
pub const VERIFIABLE_SUMMARIES: &[&str] = &["CaughtMutant", "MissedMutant", "Timeout"];

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

/// 变异基线（`tools/baselines/mutants-baseline.json`）：本轮不判红的 missed 键白名单。
///
/// 公开是为了让 `tier_verdicts` 能被单测直接喂数据——不公开就只能靠一次 17 分钟真跑验判据。
#[derive(Debug, Deserialize)]
pub struct Baseline {
    /// 已登记的未捕获变异键（`文件::函数`），棘轮只准减。
    pub missed: Vec<String>,
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

/// 这一轮要不要做"基线键可验证性"核对：只有全档要。
///
/// 增量档的面天然小于基线（`maturity.rs::seg_eq` 这轮可能压根没被改到），拿全档的尺量半张面
/// 会把"没跑到"报成"不可验证"——一片里每轮都红，红就又成了噪声（DD-0007 的形状）。所以增量档
/// 只判**新出现**的存活变异；可验证性由收尾那轮全档关账。
pub fn checks_verifiability(since: Option<&str>) -> bool {
    since.is_none()
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

/// 查 `path` 所在盘的可用字节 + 拿到数的通道名（`none` = 两条通道都没给数）。
///
/// 全仓 `unsafe_code = "deny"`，所以不直接调 `GetDiskFreeSpaceExW`，改问原生命令：
/// Windows 先用 PowerShell 的 `Get-PSDrive`（系统自带，不依赖 Git Bash 的 PATH），
/// 没拿到再退 `df -kP`；非 Windows 只有 `df -kP`。通道名如实播报——让人看得出数从哪来。
pub fn free_bytes_probe(path: &Path) -> (Option<u64>, &'static str) {
    #[cfg(windows)]
    if let Some(letter) = drive_letter(path)
        && let Some(bytes) = free_bytes_via_powershell(&letter)
    {
        return (Some(bytes), "powershell");
    }
    match free_bytes_via_df(path) {
        Some(bytes) => (Some(bytes), "df"),
        None => (None, "none"),
    }
}

/// `Get-PSDrive <盘符>`.Free 的参数（拼错参数会被断言打死，不靠真跑一次才发现）。
pub fn powershell_args(letter: &str) -> Vec<String> {
    vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-Command".to_string(),
        format!("(Get-PSDrive -Name {letter}).Free"),
    ]
}

/// `df -kP <path>` 的参数（`-P` = POSIX 输出格式，保证一个文件系统一行）。
pub fn df_args(path: &Path) -> Vec<String> {
    vec!["-kP".to_string(), path.to_string_lossy().into_owned()]
}

/// PowerShell 通道的字节数。
fn free_bytes_via_powershell(letter: &str) -> Option<u64> {
    let out = std::process::Command::new("powershell")
        .args(powershell_args(letter))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_u64_lines(&out.stdout)
}

/// `df` 通道的字节数。
pub fn free_bytes_via_df(path: &Path) -> Option<u64> {
    let out = std::process::Command::new("df")
        .args(df_args(path))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_df_avail(&String::from_utf8_lossy(&out.stdout))
}

/// `df -kP` 输出里最后一个非空行的 Available 列（1K 块）→ 字节。
pub fn parse_df_avail(text: &str) -> Option<u64> {
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    line.split_whitespace()
        .nth(3)?
        .parse::<u64>()
        .ok()
        .map(|kb| kb * 1024)
}

/// 命令 stdout 里第一个纯数字行（PowerShell 的 `Get-PSDrive`.Free 是单个数 + CRLF）。
pub fn parse_u64_lines(raw: &[u8]) -> Option<u64> {
    String::from_utf8_lossy(raw)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()))?
        .parse()
        .ok()
}

/// 盘量不足时报什么（纯函数：TMP 路径、实测余量、下限给定）。够或查不到 → `None`。
pub fn short_message(scratch: &Path, free: Option<u64>, min_gib: u64) -> Option<String> {
    if !scratch_is_short(free, min_gib) {
        return None;
    }
    Some(format!(
        "TMP 所在盘余量不足（{} < {min_gib}GB）：{} 是 cargo-mutants 开 scratch 的地方，盘满时链接失败只判 unviable，门会缺测。一行修法：把 TMP 指到宽裕的盘再跑，例如 TMP=D:/tmp/adv-mut TEMP=D:/tmp/adv-mut cargo run -p xtask -- mutants",
        human_bytes(free),
        scratch.display()
    ))
}

/// 跑之前的盘量预检：查实测余量，不足则红。
pub fn precheck_scratch(min_gib: u64) -> Vec<String> {
    let scratch = std::env::temp_dir();
    let (free, channel) = free_bytes_probe(&scratch);
    println!(
        "预检 TMP={} 余量={} 通道={channel} 下限={min_gib}GB",
        scratch.display(),
        human_bytes(free)
    );
    short_message(&scratch, free, min_gib)
        .map(|msg| vec![msg])
        .unwrap_or_default()
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

fn git_diff_patch(root: &Path, spec: &str) -> Result<PathBuf> {
    let patch = root.join("target/mutants-diff.patch");
    let out = std::process::Command::new("git")
        .args(["diff", "--binary", spec])
        .current_dir(root)
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "diff 口径 {spec} 取不到（git 退出码 {:?}）：{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    std::fs::create_dir_all(patch.parent().expect("parent"))?;
    std::fs::write(&patch, &out.stdout)?;
    Ok(patch)
}

/// 变异面用哪个 diff 口径：**默认全档**（`base...HEAD` 的全部改动），`--since` 才走增量。
///
/// 为什么增量用两点 `since..HEAD` 而不是三点：三点会退回 merge-base，把"上次绿之后
/// 我又改了 400 行"重新算成整个片的面——那正是这笔债要省的墙钟（DD-0004 实测：单轮
/// 17–40 分钟，一片里跑了 8 轮，其中 6 轮只需要看当轮改动的 15 个文件里的一部分）。
pub fn diff_spec(base: &str, since: Option<&str>) -> String {
    match since {
        Some(s) => format!("{s}..HEAD"),
        None => format!("{base}...HEAD"),
    }
}

/// 裁决行的名字：增量档必须**在裁决行里自称增量档**，否则"绿"会被抄成收尾绿。
/// （与 DD-0009 同一条纪律：看不见"没判"的门会骗人，看不见面目的门也一样。）
pub fn verdict_name(since: Option<&str>) -> String {
    match since {
        None => "mutants".to_string(),
        Some(s) => format!("mutants·增量档({s}..HEAD，只测本次改动·不作收尾绿)"),
    }
}

/// 增量档 + `--update` = 拒绝：整表替换会拿"只跑了一小片面"的结果去**删掉**全档才有的键。
pub fn refuse_incremental_update(since: Option<&str>, update: bool) -> Option<String> {
    (since.is_some() && update).then(|| {
        "增量档不许重录基线：本轮没跑全档，missed 集天然偏小，整表替换会把全档才有的键静默删掉         （想重录请去掉 --since 跑全档）"
            .to_string()
    })
}

/// cargo-mutants 退出码里"这一轮真跑完了"的那些：0 全捕获、2 有未捕获、3 有超时。
/// 其余（1 用法错、4 基线自身红/挂、5 patch 与树不符、6 patch 非法、70 内部错）都不该
/// 被解读成门的结果。判据单列成函数：这样"把 3 当成失败"这个错法自己有反例可打。
pub fn completed_status(code: Option<i32>) -> bool {
    matches!(code, Some(0) | Some(2) | Some(3))
}

/// 超时披露文案：无超时 ⇒ None；有 ⇒ 逐键点名。
///
/// 超时不算"未捕获"（变异把测试挂住 = 测试确实会拒绝它），但上游明说另一种可能是
/// `--timeout` 定得太低，那会把存活变异藏进这一类，所以不静默。
pub fn timeout_note(timeouts: &[String]) -> Option<String> {
    (!timeouts.is_empty()).then(|| {
        format!(
            "超时 {} 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：{}",
            timeouts.len(),
            timeouts.join(", ")
        )
    })
}

/// 一档跑完的裁决材料：违规清单 + 一行"档"说明。
///
/// 抽成纯函数的两个理由：① `run` 已经涨到硬阈边缘（god 门 120 行），把判定搬出来才是本仓
/// 一贯的做法；② 增量档那条"不判红但要播报"的分支只有在这里才可测——不然它永远靠一次
/// 17 分钟的真跑才能验，那种判据等于没判据。
pub fn tier_verdicts(
    since: Option<&str>,
    baseline: &Baseline,
    missed: &[String],
    verifiable: &[String],
    disk: Vec<String>,
) -> (Vec<String>, Option<String>) {
    let mut violations: Vec<String> = new_missed(missed, &baseline.missed)
        .into_iter()
        .map(|k| format!("新增未捕获变异：{k}"))
        .collect();
    let unverifiable = unverifiable_keys(&baseline.missed, verifiable);
    let mut note = None;
    if checks_verifiability(since) {
        violations.extend(unverifiable.into_iter().map(|k| {
            format!(
                "基线键本轮无可判定变异（不可验证；多为变异全判 unviable 或构建/链接失败）：{k}"
            )
        }));
    } else if !unverifiable.is_empty() {
        // 增量档不拿它判红（半张面量不到全档的键是必然的），但要把数报出来，免得
        // "增量绿"被读成"账都核过了"——关账的是收尾那轮全档。
        note = Some(format!(
            "增量档不核对基线可验证性：基线 {} 键里 {} 个不在本轮面内（收尾必须再跑全档关账）",
            baseline.missed.len(),
            unverifiable.len()
        ));
    }
    violations.extend(disk);
    (violations, note)
}

/// 跑变异门。`base` 为 diff 基线 ref；`update` 重录基线（披露通道）。
pub fn run(
    root: &Path,
    base: &str,
    since: Option<&str>,
    update: bool,
    timeout_secs: u64,
) -> Result<Vec<String>> {
    if let Some(why) = refuse_incremental_update(since, update) {
        return Ok(vec![why]);
    }
    let spec = diff_spec(base, since);
    let patch = git_diff_patch(root, &spec)?;
    let patch_text = std::fs::read_to_string(&patch)?;
    if patch_text.trim().is_empty() {
        return Ok(vec![format!(
            "skip: 与 {spec} 无差异，无变异面（skip 不算绿——门在此档视为通过并留痕）"
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
    // cargo-mutants 退出码契约（上游文档 mutants.rs/exit-codes.html，27.1.0 实测一致）：
    // 0 = 全部被捕获；2 = 有未捕获变异；3 = 有变异超时。**三码都算"跑完了"**，正文一律
    // 进棘轮比对。1/4/5/6/70（用法错 / 基线本身就红或挂 / patch 与树不符 / patch 非法 /
    // 内部错）才是真失败——判红而不解读结果。
    // 这一条是 2026-10-06 补的：旧判据只认 0|2，于是 `seg_eq` 里两个 `+= → *=` 死循环
    // 触发 exit 3，整道门在比对前就中止，连着两轮把"门绿"报成了 cargo-mutants 的转储。
    anyhow::ensure!(
        completed_status(out.status.code()),
        "cargo mutants 没跑完（exit {:?}）：{}",
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
    println!(
        "变异面 总={total} 捕获={caught} 未捕获={missed_n} unviable={unviable_n} 档={}",
        if since.is_some() { "增量" } else { "全档" }
    );
    if unviable_n > 0 {
        println!(
            "提示：unviable {unviable_n} 条的日志在 {}/mutants.out/log/；条数增多先怀疑 scratch 盘满/链接失败，别把缺测读成没问题",
            out_dir.display()
        );
    }
    let disk = unviable_disk_failures(&parsed, &out_dir);
    let timeouts = keys_by_summary(&parsed, |s| s == "Timeout")?;
    if let Some(note) = timeout_note(&timeouts) {
        println!("{note}");
    }
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
    let (violations, note) = tier_verdicts(since, &baseline, &missed, &verifiable, disk);
    if let Some(line) = note {
        println!("{line}");
    }
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
