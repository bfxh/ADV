//! 变异门的**轮转窗口**档（DD-0017 的修法）：一圈跑完 = 顶一轮全档。
//!
//! 为什么需要这一档（当场量出来的，不是推测）：`cargo mutants --list --in-diff <全档 patch>`
//! 在 2026-10-10 量出本分支全档面 = **1297 条变异 / 49 个文件**（sub-second，不编译）。
//! 一次跑完它在这台机器上兑现不了，但**堵的是墙钟不是盘**——登记 DD-0017 时我按"scratch 随
//! 变异数线性增长（~116MB/条）"外推过 8000 条 ≈ 900GB，那个尺度和机制都错了，实测在这里纠正：
//! 41 条的 adv-index 窗口全程峰值只比跑前多 **1.38GB**，且增量几乎全在建阶段（第 10 条到第 40 条
//! 只涨约 0.09GB ⇒ 平台段约 3MB/条）；主导项是**每个 worker 一份包构建**（j=4 时 4 份），
//! 工作区 `target/` 本体 11.3GB ⇒ 重包窗口才是吃盘的那一类，旋钮是 `-j` 不是窗口大小。
//! 窗口按变异预算切，买的是"一窗 ≤ 十分钟级 + 一圈可核"，见下。
//!
//! 轮转档把那句话换成可核对的东西：**开圈时定格变异面**（`face`），每轮按变异预算取一批文件、
//! 用 git pathspec 把 patch 收窄到这些文件（实测收窄是精确的：`bm25.rs + taint.rs` 两文件
//! 收窄后 listing = 209 条 = 141+68，与全档面里这两个文件的条数逐一对上），游标落
//! [`ROTATION_PATH`]。关账条件写死成两条：① `face` 里每个文件都进过窗 ⇒ 各窗并集 = 全档面；
//! ② 基线每个键在这一圈里都被判过。两条都在同一轮真跑之后才判，不靠事后补记。
//!
//! 判据一条没放松：新 missed 照样红、资源缺测照样红，且共用 [`mutants::run_cargo_mutants`]
//! 这一条读数路径——分档只改"喂哪个 patch 或哪份文件清单"，不许各写一套解析。
//!
//! 中途来了新提交怎么办：**重开一圈并点名**（旧证据不再等于当前面，不拿它关账）。为此面本身
//! 存进游标文件，而不是每轮重算需求。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::mutants;

/// 轮转游标（相对工作区根）：本圈定格的面 + 已进窗的文件 + 已判过的键。
pub const ROTATION_PATH: &str = "tools/baselines/mutants-rotation.json";

/// 每窗变异预算默认值：150 是按**实测耗时**定的（2026-10-10 adv-index 窗口实测
/// "41 mutants tested in 2m"，j=4 ⇒ 约 3s/条 ⇒ 一窗约 8 分钟测试 + 一次冷构建），
/// 不是按盘量定的——盘量的主导项是"每个 worker 一份包构建"，见模块头的成本实测段。
pub const DEFAULT_WINDOW_BUDGET: usize = 150;

/// 单条变异的 scratch 投影（字节）：2026-10-10 实测**平台段约 3MB/条**
/// （41 条的窗口峰值 +1.38GB，且增量几乎全在建阶段；从第 10 条到第 40 条只涨约 0.09GB），
/// 这里取 16MB 留 ≥5× 头寸。它拦的是"预算被配成几千条"的病态窗口，不是日常。
pub const SCRATCH_BYTES_PER_MUTANT: u64 = 16 * 1024 * 1024;

/// 本窗计划：取哪些文件、投影多少条变异、还剩哪些没进窗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// 本窗的文件（pathspec 直接用它们）。
    pub files: Vec<String>,
    /// 本窗的变异条数（来自 listing，跑之前就知道）。
    pub mutants: usize,
    /// 单文件就超预算时点名（照样开窗，否则这个文件永远进不了圈）。
    pub over_budget: Option<String>,
}

/// 一圈的证据（JSON 落盘，`tools/baselines/` 里可核）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lap {
    /// 本圈的 diff 口径（`base...HEAD`）。口径变了 ⇒ 旧证据作废重开。
    pub spec: String,
    /// 开窗预算（记下来让"这圈为什么切成这样"可追）。
    pub budget: usize,
    /// 开圈时**定格**的变异面：文件 → 该文件变异条数，按文件名排序。
    pub face: Vec<(String, usize)>,
    /// 已进过窗的文件。
    pub covered_files: Vec<String>,
    /// 本圈累计"跑到判定环节"的键（跨窗并集）——关账拿它对基线。
    pub covered_keys: Vec<String>,
    /// 本圈累计的未捕获键（并集）：一圈跑完这就是全档 missed 集，`--update` 才有资格整表重录。
    pub missed_keys: Vec<String>,
    /// 已跑的窗数。
    pub rounds: usize,
    /// 其中"键缺口专用窗"数（DD-0017 A 案）：整文件开窗判"diff 口径取不到变异"的
    /// 基线键。旧游标没有这个字段 ⇒ `serde(default)` 读成 0，不许读挂。
    #[serde(default)]
    pub key_gap_rounds: u32,
    /// 本圈里出现资源缺测（盘满/内存）的窗数：非 0 就不许拿这一圈重录基线。
    pub resource_rounds: u32,
    /// 历史已关账的圈数（关账后 `face/covered_*` 清零，这个数继续涨）。
    pub completed_laps: u32,
}

/// 一条 `cargo mutants --list` 文本行 → 文件名。
///
/// 形状是 `path:line:col: 描述`；只认 line/col 全是数字的行，cargo 自己的提示行一律返回 `None`
/// （不猜）。描述里再出现冒号不影响：`splitn(4)` 把剩下的整段留给描述。
pub fn listing_file(line: &str) -> Option<String> {
    let mut parts = line.splitn(4, ':');
    let file = parts.next()?.trim();
    if file.is_empty() {
        return None;
    }
    let line_no = parts.next()?.trim();
    let col = parts.next()?.trim();
    let digit = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if digit(line_no) && digit(col) {
        Some(file.to_string())
    } else {
        None
    }
}

/// listing 文本 → 按文件名排序的 `(文件, 变异数)`。
pub fn file_counts(listing: &str) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for line in listing.lines() {
        if let Some(file) = listing_file(line) {
            *counts.entry(file).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

/// 还没进过窗的面文件（保持面里的排序 = 文件名序，确定性）。
pub fn uncovered(face: &[(String, usize)], covered: &[String]) -> Vec<(String, usize)> {
    face.iter()
        .filter(|(file, _)| !covered.contains(file))
        .cloned()
        .collect()
}

/// 本窗取哪些文件：按面顺序贪心到预算为止，**至少取 1 个**。
///
/// "至少取 1 个"是防死锁：某个文件自己就超预算时，拒绝开窗会让它永远进不了圈，
/// 于是这一圈永远关不了账——那正是本档要消灭的状态。超没超由 [`Window::over_budget`] 说。
pub fn pick_window(face: &[(String, usize)], covered: &[String], budget: usize) -> Window {
    let mut files = Vec::new();
    let mut total = 0usize;
    for (file, count) in uncovered(face, covered) {
        if !files.is_empty() && total + count > budget {
            break;
        }
        total += count;
        files.push(file);
    }
    let over_budget = if total > budget && files.len() == 1 {
        Some(format!(
            "单文件 {} 就有 {total} 条变异，超过窗口预算 {budget}——照样开窗（不然它永远进不了圈）",
            files[0]
        ))
    } else {
        None
    };
    Window {
        files,
        mutants: total,
        over_budget,
    }
}

/// 面在圈中变了没有：定格面与当前面不等就点名（差多少文件/条）。
pub fn face_drift(recorded: &[(String, usize)], live: &[(String, usize)]) -> Option<String> {
    if recorded == live {
        return None;
    }
    let sum = |face: &[(String, usize)]| face.iter().map(|(_, c)| c).sum::<usize>();
    Some(format!(
        "变异面在圈中变了（定格 {} 文件/{} 条 → 当前 {} 文件/{} 条）：旧证据不再等于当前面，重开一圈",
        recorded.len(),
        sum(recorded),
        live.len(),
        sum(live)
    ))
}

/// 关账条件①：面里还没进窗的文件。
pub fn file_gaps(face: &[(String, usize)], covered: &[String]) -> Vec<String> {
    uncovered(face, covered)
        .into_iter()
        .map(|(f, _)| f)
        .collect()
}

/// 关账条件②：本圈没判过的基线键，按**成因**分两说。
///
/// 为什么分两说：键所在文件压根不在面上（全档口径下也产不出它的变异）和"在面上但那扇窗里
/// 变异全没跑到判定环节"是两种不同的红，混成一条会让人去查错的地方。两案现在都由键缺口
/// 专用窗接走（DD-0017 A 案），差别只剩"整文件 listing 里到底还有没有它"。
///
/// 键的文件段从**第一个** `::` 处切：文件路径（`…/bm25.rs`）自身不含 `::`，而函数段可以
/// 带好几段（`SearchIndex::search`、`<impl Analysis<'tcx> for Reach<'a>>::apply_…`）——
/// 从右切会把 `bm25.rs::SearchIndex` 当文件（金丝雀首跑抓到的真错，旧版两处都带着）。
pub fn key_gaps(baseline: &[String], face: &[(String, usize)], lap: &Lap) -> Vec<String> {
    let face_files: Vec<&str> = face.iter().map(|(f, _)| f.as_str()).collect();
    baseline
        .iter()
        .filter(|k| !lap.covered_keys.iter().any(|c| c == *k))
        .map(|k| {
            let file = k.split_once("::").map(|(f, _)| f).unwrap_or(k.as_str());
            if face_files.contains(&file) {
                format!(
                    "基线键 {k} 本圈没判到（所在文件进了窗却没产出它的可判定变异）：\
                     下一轮键缺口专用窗会对 {file} 整文件开窗（DD-0017 A 案）；若还不行查那扇窗的 unviable 日志"
                )
            } else {
                format!(
                    "基线键 {k} 所在文件不在变异面上：下一轮键缺口专用窗会整文件开窗判它\
                     （DD-0017 A 案，覆盖只变大）；若整文件 listing 也没有它，键随代码消失了就划账"
                )
            }
        })
        .collect()
}

/// 一圈没闭合就想 `--update` = 拒绝：拿半圈的 missed 整表替换会把只在别处出现的键静默删掉。
pub fn refuse_partial_update(closing: bool, update: bool) -> Option<String> {
    (!closing && update).then(|| {
        "轮转档不许在未关账时重录基线：本圈面还没覆盖全，整表替换会删掉别的窗口里才有的键         （想重录请跑到本圈关账那一轮再带 --update）"
            .to_string()
    })
}

/// 本窗投影的 scratch 与实测余量对不上时怎么说（余量查不到 ⇒ 不判红，与全档那条预检同一口径）。
pub fn window_scratch_message(
    free_bytes: Option<u64>,
    window_mutants: usize,
    channel: &str,
) -> Option<String> {
    let free = free_bytes?;
    let projected = u64::try_from(window_mutants)
        .ok()?
        .saturating_mul(SCRATCH_BYTES_PER_MUTANT);
    if free >= projected {
        return None;
    }
    Some(format!(
        "本窗 {window_mutants} 条变异的 scratch 投影约 {}GB，TMP 所在盘实测只剩 {}GB（通道={channel}）：\
         降预算再来，例如 ADV_MUTANTS_WINDOW_BUDGET={} …；投影是**防跑到一半 ENOSPC 缺测**，不是判据",
        projected / (1024 * 1024 * 1024),
        free / (1024 * 1024 * 1024),
        (free / SCRATCH_BYTES_PER_MUTANT).max(1)
    ))
}

/// 关账那一轮该说什么（同时把进度算出来，让"还剩多少"不必人肉数）。
pub fn progress_line(face: &[(String, usize)], lap: &Lap, baseline: &[String]) -> String {
    let judged = baseline
        .iter()
        .filter(|k| lap.covered_keys.iter().any(|c| c == *k))
        .count();
    format!(
        "未关账：本圈 {}/{} 文件已进窗（{} 窗），基线键已判 {}/{}",
        lap.covered_files.len(),
        face.len(),
        lap.rounds,
        judged,
        baseline.len()
    )
}

/// 新开一圈。
pub fn open_lap(spec: &str, budget: usize, face: &[(String, usize)], completed: u32) -> Lap {
    Lap {
        spec: spec.to_string(),
        budget,
        face: face.to_vec(),
        covered_files: Vec::new(),
        covered_keys: Vec::new(),
        missed_keys: Vec::new(),
        rounds: 0,
        key_gap_rounds: 0,
        resource_rounds: 0,
        completed_laps: completed,
    }
}

/// 定格面的变异总数（关账文案里"覆盖全档面 N 条"就是它）。
pub fn face_total(face: &[(String, usize)]) -> usize {
    face.iter().map(|(_, count)| count).sum()
}

/// 把一轮的证据并进游标（并集排序去重，保证同一圈跑两次得到同一份 JSON）。
pub fn record(lap: &mut Lap, window: &Window, evidence: &mut mutants::RoundEvidence) {
    if !evidence.resource.is_empty() {
        lap.resource_rounds += 1;
    }
    for file in &window.files {
        if !lap.covered_files.contains(file) {
            lap.covered_files.push(file.clone());
        }
    }
    lap.covered_files.sort();
    for key in evidence.verifiable.drain(..) {
        if !lap.covered_keys.contains(&key) {
            lap.covered_keys.push(key);
        }
    }
    lap.covered_keys.sort();
    for key in evidence.missed.drain(..) {
        if !lap.missed_keys.contains(&key) {
            lap.missed_keys.push(key);
        }
    }
    lap.missed_keys.sort();
    lap.rounds += 1;
}

/// 读游标（不存在 = 还没开过圈，返回 `None` 让调用方开新圈；坏 JSON 一律红，不猜）。
pub fn load_lap(path: &Path) -> Result<Option<Lap>> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(Some(
            serde_json::from_str(&raw).with_context(|| format!("解析 {}", path.display()))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("读 {}", path.display()))),
    }
}

/// 写游标（排序键已在构造时保证，这里只保证落盘形状稳定：pretty + 定长字段序）。
pub fn save_lap(path: &Path, lap: &Lap) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(std::fs::write(
        path,
        serde_json::to_string_pretty(lap)? + "\n",
    )?)
}

/// 轮转档的档名：**绿在名字里就带着"这是一扇窗"**，防止被抄成收尾全档绿。
/// 关账那一轮由 [`run`] 另打一行"本圈关账"。
pub fn verdict_name(budget: usize) -> String {
    format!("mutants·轮转档(每窗≤{budget}变异·一圈覆盖全档面+全部基线键才关账)")
}

/// `cargo mutants --list <限定>` → 面（实测 sub-second，不编译，所以每轮都敢重算）。
/// 限定 = `--in-diff <patch>`（diff 面）或一串 `--file <f>`（键缺口窗的整文件面）。
fn list_face(root: &Path, limit: &[String]) -> Result<Vec<(String, usize)>> {
    let mut args: Vec<String> = vec!["mutants".into(), "--list".into()];
    args.extend(limit.iter().cloned());
    let out = std::process::Command::new("cargo")
        .args(&args)
        .current_dir(root)
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "--list 取面失败（exit {:?}）：{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(file_counts(&String::from_utf8_lossy(&out.stdout)))
}

/// 本圈还没判过的基线键 → 所在文件（去重、排序）。键缺口专用窗按它取材。
///
/// 键的文件段从**第一个** `::` 处切：文件路径（`…/bm25.rs`）自身不含 `::`，而函数段可以
/// 带好几段（`SearchIndex::search`、`<impl Analysis<'tcx> for Reach<'a>>::apply_…`）——
/// 从右切会把 `bm25.rs::SearchIndex` 当文件（金丝雀首跑抓到的真错，旧版两处都带着）。
pub fn key_gap_files(baseline: &[String], lap: &Lap) -> Vec<String> {
    let mut files: Vec<String> = baseline
        .iter()
        .filter(|k| !lap.covered_keys.iter().any(|c| c == *k))
        .map(|k| {
            k.split_once("::")
                .map(|(f, _)| f)
                .unwrap_or(k.as_str())
                .to_string()
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

/// 整文件口径的 listing 限定：`["--file", f, "--file", g, …]`。
fn file_limit(files: &[String]) -> Vec<String> {
    let mut limit = Vec::new();
    for file in files {
        limit.push("--file".to_string());
        limit.push(file.clone());
    }
    limit
}

/// 接着上一圈跑还是重开一圈：口径变了或面漂移就**点名**后重开，已关账圈数保留。
///
/// 为什么面要"定格"而不是每轮重算需求：一圈要跨多次调用（甚至跨提交），每轮重算会让
/// "一圈覆盖全档面"变成移动球门。定格的面对面会漂，漂了就作废重跑——宁可不关账，
/// 也不拿旧证据顶新面。
fn resume_lap(stored: Option<Lap>, spec: &str, budget: usize, live: &[(String, usize)]) -> Lap {
    match stored {
        Some(lap) if lap.spec == spec => match face_drift(&lap.face, live) {
            Some(why) => {
                println!("{why}");
                open_lap(spec, budget, live, lap.completed_laps)
            }
            None => Lap { budget, ..lap },
        },
        Some(lap) => {
            println!(
                "游标口径是 {}，本轮口径是 {spec} ⇒ 旧证据不顶用，重开一圈（已关账 {} 圈保留计数）",
                lap.spec, lap.completed_laps
            );
            open_lap(spec, budget, live, lap.completed_laps)
        }
        None => open_lap(spec, budget, live, 0),
    }
}

/// 关账那一轮的固定动作：点名 ⇒ 有资格才重录基线 ⇒ 开新圈（覆盖清零、圈数保留、面原样带走）。
fn close_lap(
    lap: &mut Lap,
    spec: &str,
    budget: usize,
    baseline_path: &Path,
    baseline_keys: usize,
    update: bool,
    violations: &mut Vec<String>,
) -> Result<()> {
    println!(
        "本圈关账：{} 窗覆盖全档面 {} 条变异 / {} 文件，基线 {baseline_keys} 键全部判过 ⇒ 这一圈顶一轮全档{}",
        lap.rounds,
        face_total(&lap.face),
        lap.face.len(),
        if lap.key_gap_rounds > 0 {
            format!(
                "（其中键缺口专用窗 {} 扇，判的是 diff 面取不到的键）",
                lap.key_gap_rounds
            )
        } else {
            String::new()
        }
    );
    if update {
        let resource = if lap.resource_rounds > 0 {
            vec![format!(
                "本圈 {} 窗出现资源缺测（盘满/内存）：缺测不能当成已清，基线不动",
                lap.resource_rounds
            )]
        } else {
            Vec::new()
        };
        violations.extend(mutants::write_baseline(
            baseline_path,
            &lap.missed_keys,
            &resource,
        )?);
    }
    let completed = lap.completed_laps + 1;
    let face = std::mem::take(&mut lap.face);
    *lap = open_lap(spec, budget, &face, completed);
    Ok(())
}

/// 一窗跑完后的裁决：关账 / 键缺口点名 / 进度播报，违规并进 `violations`。
fn settle_round(
    lap: &mut Lap,
    spec: &str,
    budget: usize,
    baseline: &mutants::Baseline,
    baseline_path: &Path,
    update: bool,
    violations: &mut Vec<String>,
) -> Result<()> {
    let closing = file_gaps(&lap.face, &lap.covered_files).is_empty();
    let gaps = if closing {
        key_gaps(&baseline.missed, &lap.face, lap)
    } else {
        Vec::new()
    };
    if closing && gaps.is_empty() {
        close_lap(
            lap,
            spec,
            budget,
            baseline_path,
            baseline.missed.len(),
            update,
            violations,
        )?;
    } else if closing {
        println!("面已全覆盖，但基线键还没判齐（本圈不能关账）：");
        for gap in &gaps {
            println!("  - {gap}");
        }
        violations.extend(gaps);
        if let Some(why) = refuse_partial_update(false, update) {
            violations.push(why);
        }
    } else {
        println!("{}", progress_line(&lap.face, lap, &baseline.missed));
        if let Some(why) = refuse_partial_update(false, update) {
            violations.push(why);
        }
    }
    Ok(())
}

/// 跑一扇窗前的两道盘量线：全档那条体验线（`precheck_scratch`）+ 本窗投影。
/// 非空即"这窗先别跑"，且此时**游标一个字都不改**（没跑过就不能记成跑过）。
fn preflight(window: &Window) -> Vec<String> {
    let min_gib = mutants::min_free_gib(std::env::var("ADV_MUTANTS_MIN_FREE_GIB").ok());
    let mut short = mutants::precheck_scratch(min_gib);
    let scratch = mutants::scratch_dir();
    let (free, channel) = mutants::free_bytes_probe(&scratch);
    if let Some(why) = window_scratch_message(free, window.mutants, channel) {
        short.push(why);
    }
    short
}

/// 面已全覆盖时补判基线键的窗（DD-0017 A 案，2026-10-10 用户拍板"选 A"）：
/// 对判不到的键**整文件开窗**（`--file`，不带 `--in-diff`）。函数没被本分支改过 ⇒
/// diff 面取不到它的变异，这是结构性判不到，不是"没问题"；整文件窗多判的全是
/// diff 面外的变异，覆盖只变大不缩小。
///
/// 返回 `(窗, 死路违规)`：有缺口 ⇒ 预算切好的窗 + 空违规；缺口存在但整文件 listing
/// 一条变异都没有 ⇒ 键随代码消失了，返回让 `--update` 披露通道去划账的死路文案；
/// 连缺口都空（游标该关账没关）⇒ fail-loud。
fn key_gap_window(
    root: &Path,
    lap: &Lap,
    baseline_missed: &[String],
    budget: usize,
) -> Result<(Window, Vec<String>)> {
    let gap_files = key_gap_files(baseline_missed, lap);
    anyhow::ensure!(
        !gap_files.is_empty(),
        "面已全覆盖且基线键已判齐，游标却未关账——状态矛盾，查 {ROTATION_PATH}"
    );
    let gap_face = list_face(root, &file_limit(&gap_files))?;
    let window = pick_window(&gap_face, &[], budget);
    if window.files.is_empty() {
        return Ok((
            window,
            vec![format!(
                "键缺口专用窗没活可干：{} 的整文件 listing 一条变异都没有——\
                 键随代码消失了就走 --update 的披露通道划账，否则人工分诊",
                gap_files.join(", ")
            )],
        ));
    }
    Ok((window, Vec::new()))
}

/// 跑一扇窗（轮转档入口）。返回违规清单；游标在判定之后落盘。
pub fn run(
    root: &Path,
    base: &str,
    budget: usize,
    update: bool,
    timeout_secs: u64,
) -> Result<Vec<String>> {
    let spec = mutants::diff_spec(base, None);
    let full_patch = mutants::git_diff_patch(root, &spec, &[])?;
    let live_face = list_face(
        root,
        &[
            "--in-diff".to_string(),
            full_patch.to_string_lossy().into_owned(),
        ],
    )?;
    if live_face.is_empty() {
        return Ok(vec![format!(
            "skip: 与 {spec} 无变异面（skip 不算绿——门在此档视为通过并留痕）"
        )]);
    }
    let baseline_path = root.join(mutants::BASELINE_PATH);
    let baseline = mutants::read_baseline(&baseline_path)?;

    let rotation_path = root.join(ROTATION_PATH);
    let mut lap = resume_lap(load_lap(&rotation_path)?, &spec, budget, &live_face);

    let mut window = pick_window(&lap.face, &lap.covered_files, budget);
    let mut key_gap = false;
    if window.files.is_empty() {
        let (gap_window, gap_dead) = key_gap_window(root, &lap, &baseline.missed, budget)?;
        if !gap_dead.is_empty() {
            return Ok(gap_dead);
        }
        key_gap = true;
        let unjudged = baseline
            .missed
            .iter()
            .filter(|k| !lap.covered_keys.contains(k))
            .count();
        println!(
            "键缺口专用窗：基线还有 {unjudged} 键没判到，对 {} 整文件开窗（覆盖只变大不缩小）",
            gap_window.files.join(", ")
        );
        window = gap_window;
    }
    if let Some(why) = &window.over_budget {
        println!("注意：{why}");
    }
    println!(
        "本窗 {} 个文件 / {} 条变异（预算 {budget}）",
        window.files.len(),
        window.mutants
    );

    let short = preflight(&window);
    if !short.is_empty() {
        // 没真跑 ⇒ 游标一个字不改（记了就成了"跑过的窗"）。
        return Ok(short);
    }

    let (scope, tier) = if key_gap {
        (mutants::Scope::Files(window.files.clone()), "轮转·键缺口")
    } else {
        (
            mutants::Scope::Diff(mutants::git_diff_patch(root, &spec, &window.files)?),
            "轮转",
        )
    };
    let mut evidence = mutants::run_cargo_mutants(root, scope, timeout_secs, tier)?;

    let mut violations: Vec<String> = mutants::new_missed(&evidence.missed, &baseline.missed)
        .into_iter()
        .map(|k| format!("新增未捕获变异：{k}"))
        .collect();
    violations.extend(evidence.resource.clone());

    record(&mut lap, &window, &mut evidence);
    if key_gap {
        lap.key_gap_rounds += 1;
    }
    settle_round(
        &mut lap,
        &spec,
        budget,
        &baseline,
        &baseline_path,
        update,
        &mut violations,
    )?;
    save_lap(&rotation_path, &lap)?;
    println!(
        "游标已写 {ROTATION_PATH}（第 {} 窗，累计覆盖 {} 文件 / 判过 {} 键）",
        lap.rounds,
        lap.covered_files.len(),
        lap.covered_keys.len()
    );
    Ok(violations)
}

/// `ADV_MUTANTS_WINDOW_BUDGET` 的读法：坏值退回默认并点名（不静默放大窗口）。
pub fn window_budget(raw: Option<&str>) -> (usize, Option<String>) {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => (DEFAULT_WINDOW_BUDGET, None),
        Some(s) => match s.parse::<usize>() {
            Ok(0) | Err(_) => (
                DEFAULT_WINDOW_BUDGET,
                Some(format!(
                    "ADV_MUTANTS_WINDOW_BUDGET={s} 不是正整数，退回默认 {DEFAULT_WINDOW_BUDGET}"
                )),
            ),
            Ok(n) => (n, None),
        },
    }
}
