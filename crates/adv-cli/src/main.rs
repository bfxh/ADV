//! ADV CLI 入口（M1 片1：`adv scan <目录>`；index/serve/gate 随里程碑落地）。
//!
//! 输出契约（C2/X15 裁定）：stdout 机器可读 JSONL（每行一条发现），stderr 人类摘要；
//! 退出码 0=扫描完成（发现的多少不改变退出码——报告与门禁是两回事，棘轮门另接）。
//!
//! 片A3 加 `--engine`：`ast`（默认，快轨 tree-sitter）/ `mir`（深轨边车子进程）/
//! `both`（两轨都跑，stdout 出并集、stderr 出三方账）。行契约两轨共用
//! `adv_rules::Finding`，不做第二套 schema。

mod mir;

use adv_rules::Finding;
use std::path::{Path, PathBuf};

/// 扫描引擎档位。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Engine {
    Ast,
    Mir,
    Both,
}

/// 一趟扫描的累计量。
#[derive(Default)]
struct Tally {
    files: usize,
    fast: Vec<Finding>,
    deep: Vec<Finding>,
    /// 深轨跑失败的文件数——失败不折算为"无发现"。
    deep_failed: usize,
}

impl Tally {
    /// 记一个"深轨没跑成"的文件。计数是退出码 2 的唯一依据，所以它得能被单独观察。
    fn note_deep_failure(&mut self) {
        self.deep_failed += 1;
    }
}

/// 快轨在哪些档跑：纯 mir 档不该带上 tree-sitter 的结果。
fn fast_runs_for(engine: Engine) -> bool {
    engine != Engine::Mir
}

/// 深轨在哪些档跑：ast 档是"只跑快轨"，与 `--engine mir` 互斥。
fn deep_runs_for(engine: Engine) -> bool {
    engine != Engine::Ast
}

/// 深轨有文件没跑成时以什么码收口（`None` = 照常 0）。
///
/// 这条判据的存在理由：深轨跑不动是**执行失败**，不是"该仓干净"。
fn deep_failure_exit(engine: Engine, deep_failed: usize) -> Option<i32> {
    (deep_runs_for(engine) && deep_failed > 0).then_some(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => scan(&args[1..]),
        _ => {
            eprintln!(
                "用法：adv scan <目录> [--rules <规则目录>] [--engine ast|mir|both] \
                 [--driver <路径>] [--deep-via-cargo]"
            );
            std::process::exit(2);
        }
    }
}

fn scan(args: &[String]) {
    let Some(target) = args.first() else {
        eprintln!("scan 需要一个目录参数");
        std::process::exit(2);
    };
    let rules_dir = flag_value(args, "--rules")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rules"));
    // 作用域排除（M1 片1 真实仓裁定：vendor/ 第三方代码不归本仓规则管）——按路径组件匹配
    let excludes: Vec<String> = args
        .windows(2)
        .filter(|w| w[0] == "--exclude")
        .map(|w| w[1].clone())
        .collect();
    let engine = engine_of(args);
    let driver = match engine {
        Engine::Ast => None,
        _ => match mir::locate_driver(flag_value(args, "--driver")) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("深轨不可用：{e}");
                std::process::exit(2);
            }
        },
    };
    let rules = match adv_rules::load_rules(&rules_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("规则加载失败：{e}");
            std::process::exit(2);
        }
    };

    let mut tally = Tally::default();
    // cargo 档（片B2）：深轨不逐文件起驱动，而是把边车挂进目标 crate 的 cargo 构建一次跑完。
    let via_cargo = args.iter().any(|a| a == "--deep-via-cargo");
    for entry in ignore::Walk::new(target) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("遍历跳过：{e}");
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() || excluded(path, &excludes) {
            continue;
        }
        let Some(lang) = language_of(path) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(path) else {
            continue; // 非 UTF-8/二进制：片1 跳过（账目见 FP 会计）
        };
        tally.files += 1;
        if fast_runs_for(engine) {
            scan_fast(path, lang, &src, &rules, &mut tally);
        }
        if deep_runs_for(engine) && !via_cargo {
            if lang == adv_parse::Language::Python {
                continue; // 深轨只吃 Rust（MIR 边车），python 文件在此档不产发现
            }
            scan_deep(
                path,
                &rules_dir,
                driver.as_ref().expect("非 ast 档必有 driver"),
                &rules,
                &mut tally,
            );
        }
    }
    if via_cargo && let Some(driver) = driver.as_ref() {
        match mir::scan_crate_via_cargo(Path::new(target), &rules_dir, driver, &rules) {
            Ok(found) => tally.deep.extend(found),
            // cargo 档跑不动 = 执行失败，不是"这个 crate 干净"（与片A2 边界口径一致）。
            Err(e) => {
                eprintln!("adv：深轨 cargo 档不可用：{e}");
                std::process::exit(2);
            }
        }
    }
    report(engine, tally);
}

/// 快轨一趟：解析 + 抑制自检 + 规则匹配。
fn scan_fast(
    path: &Path,
    lang: adv_parse::Language,
    src: &str,
    rules: &[adv_rules::Rule],
    tally: &mut Tally,
) {
    let ast = match adv_parse::parse_source(lang, src) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("解析失败 {}: {e}", path.display());
            return;
        }
    };
    let (_, malformed) = adv_rules::suppression::collect(&ast);
    for m in &malformed {
        eprintln!(
            "畸形抑制 {}:{} {}（发现照报，不静默）",
            path.display(),
            m.line,
            m.why
        );
    }
    for (line, what) in adv_rules::suppression::expired(&ast, &adv_core::today()) {
        eprintln!("抑制到期 {}:{} {what}（发现照报）", path.display(), line);
    }
    tally.fast.extend(adv_rules::run_matchers(
        &ast,
        path,
        lang,
        rules,
        &adv_core::today(),
    ));
}

/// 深轨一趟：子进程跑边车；失败进 deep_failed 清单，不当"无发现"。
fn scan_deep(
    path: &Path,
    rules_dir: &Path,
    driver: &Path,
    rules: &[adv_rules::Rule],
    tally: &mut Tally,
) {
    match mir::scan_file(driver, rules_dir, path, rules) {
        Ok(found) => tally.deep.extend(found),
        Err(e) => {
            let why = e.lines().next().unwrap_or("").to_string();
            eprintln!("深轨失败 {}: {why}", path.display());
            tally.note_deep_failure();
        }
    }
}

/// stdout 出发现（JSONL），stderr 出摘要；`both` 档再出三方账。
fn report(engine: Engine, mut tally: Tally) {
    let fast_n = tally.fast.len();
    let deep_n = tally.deep.len();
    let mut items: Vec<Finding> = Vec::with_capacity(fast_n + deep_n);
    items.append(&mut tally.fast);
    items.append(&mut tally.deep);
    let mut jsonls: Vec<String> = items.iter().map(Finding::to_jsonl).collect();
    jsonls.sort();
    for line in &jsonls {
        println!("{line}");
    }
    eprintln!(
        "adv scan：{} 个文件，{} 条发现（快轨 {fast_n} / 深轨 {deep_n}，档 {engine:?}）",
        tally.files,
        jsonls.len()
    );
    if engine != Engine::Ast {
        eprintln!(
            "  深轨：{} 个文件跑失败（失败不折算为\"无发现\"）",
            tally.deep_failed
        );
    }
    if engine == Engine::Both {
        three_way(&items);
    }
    let mut per_rule: Vec<(String, usize)> = Vec::new();
    for f in &items {
        match per_rule.iter_mut().find(|(r, _)| r == &f.rule) {
            Some((_, n)) => *n += 1,
            None => per_rule.push((f.rule.clone(), 1)),
        }
    }
    per_rule.sort();
    for (rule, n) in &per_rule {
        eprintln!("  {rule}: {n}");
    }
    // 深轨跑不动 = 执行失败，不是"该仓干净"。退出码必须红（对齐"绿=SKIP 不算绿"口径）。
    if let Some(code) = deep_failure_exit(engine, tally.deep_failed) {
        eprintln!(
            "adv：深轨有 {} 个文件未跑成，退出码 {code}（不折算为无发现）",
            tally.deep_failed
        );
        std::process::exit(code);
    }
}

/// 三方账的键：`(规则, 文件, 起始行)`。列不参与——两引擎给的列粒度口径不同
/// （快轨是 AST span，深轨是 MIR terminator span）。
type LedgerKey = (String, String, usize);

/// 三方账分桶（纯函数）：`(两边都报, 仅快轨, 仅深轨)`。
///
/// 抽出来的理由（片A6 实测）：`--engine both` 在现有 rust 语料上快轨 0 条，
/// `两边都报`/`仅快轨` 两桶恒空 ⇒ 桶逻辑的任何变异都观察不到，门会把"没覆盖"
/// 记成"已杀掉"。分桶本身必须能被直接喂数据判定。
fn three_way_buckets(items: &[Finding]) -> (Vec<LedgerKey>, Vec<LedgerKey>, Vec<LedgerKey>) {
    let key = |f: &Finding| (f.rule.clone(), f.file.clone(), f.start_line);
    let fast: Vec<LedgerKey> = items
        .iter()
        .filter(|f| f.engine == adv_rules::matcher::ENGINE_FAST)
        .map(key)
        .collect();
    let deep: Vec<LedgerKey> = items
        .iter()
        .filter(|f| f.engine == adv_rules::matcher::ENGINE_MIR)
        .map(key)
        .collect();
    let both: Vec<LedgerKey> = fast.iter().filter(|k| deep.contains(k)).cloned().collect();
    let only_fast: Vec<LedgerKey> = fast.iter().filter(|k| !both.contains(k)).cloned().collect();
    let only_deep: Vec<LedgerKey> = deep.iter().filter(|k| !both.contains(k)).cloned().collect();
    (both, only_fast, only_deep)
}

/// 三方账打印（stdout 之外的 stderr 摘要，不动 stdout 行契约）。
fn three_way(items: &[Finding]) {
    let (both, only_fast, only_deep) = three_way_buckets(items);
    eprintln!(
        "  三方账：两边都报 {} / 仅快轨 {} / 仅深轨 {}",
        both.len(),
        only_fast.len(),
        only_deep.len()
    );
    for (label, keys) in [("仅快轨", &only_fast), ("仅深轨", &only_deep)] {
        for (rule, file, line) in keys {
            eprintln!("    {label} {rule} {file}:{line}");
        }
    }
}

fn engine_of(args: &[String]) -> Engine {
    match flag_value(args, "--engine") {
        Some("mir") => Engine::Mir,
        Some("both") => Engine::Both,
        Some("ast") | None => Engine::Ast,
        Some(other) => {
            eprintln!("--engine 只认 ast|mir|both，实得 {other}");
            std::process::exit(2);
        }
    }
}

fn flag_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].as_str())
}

/// 路径任一组件命中排除名单即跳过。
fn excluded(path: &Path, excludes: &[String]) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| excludes.iter().any(|x| x == s))
    })
}

fn language_of(path: &Path) -> Option<adv_parse::Language> {
    match path.extension()?.to_str()? {
        "py" | "pyi" => Some(adv_parse::Language::Python),
        "rs" => Some(adv_parse::Language::Rust),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adv_rules::matcher::{ENGINE_FAST, ENGINE_MIR};

    const RULE: &str = "RS-TAINT-COMMAND";

    /// 造一条发现：只关心 `(engine, file, start_line)` 三元键。
    fn fnd(engine: &str, file: &str, line: usize) -> Finding {
        Finding {
            rule: RULE.to_string(),
            severity: "error".to_string(),
            message: String::new(),
            file: file.to_string(),
            start_line: line,
            start_col: 0,
            end_line: line,
            end_col: 0,
            engine: engine.to_string(),
        }
    }

    fn key(file: &str, line: usize) -> LedgerKey {
        (RULE.to_string(), file.to_string(), line)
    }

    /// 两档开关必须互斥且穷尽：`both` 两轨都跑，单档只跑自己那轨。
    #[test]
    fn engine_tiers_split_fast_and_deep() {
        assert!(
            fast_runs_for(Engine::Ast) && fast_runs_for(Engine::Both),
            "ast/both 档该跑快轨"
        );
        assert!(!fast_runs_for(Engine::Mir), "mir 档不该带上快轨");
        assert!(
            deep_runs_for(Engine::Mir) && deep_runs_for(Engine::Both),
            "mir/both 档该跑深轨"
        );
        assert!(!deep_runs_for(Engine::Ast), "ast 档不该带上深轨");
    }

    /// 深轨失败计数 → 退出码 2 这条保护的唯一入口。
    #[test]
    fn deep_failure_exit_is_red_only_where_deep_ran() {
        assert_eq!(deep_failure_exit(Engine::Mir, 0), None, "零失败不该判红");
        assert_eq!(deep_failure_exit(Engine::Mir, 3), Some(2));
        assert_eq!(deep_failure_exit(Engine::Both, 1), Some(2));
        assert_eq!(
            deep_failure_exit(Engine::Ast, 9),
            None,
            "ast 档压根没跑深轨，失败数不参与定码"
        );
    }

    /// 每个失败文件计一次（`+= 1` 变异成 `*= 1` 会让计数恒 0 ⇒ 退出码 2 静默失效）。
    #[test]
    fn each_deep_failure_counts_once() {
        let mut tally = Tally::default();
        assert_eq!(tally.deep_failed, 0, "初值不为 0 的话后面两条断言都是空的");
        tally.note_deep_failure();
        assert_eq!(tally.deep_failed, 1);
        tally.note_deep_failure();
        tally.note_deep_failure();
        assert_eq!(tally.deep_failed, 3, "深轨失败没逐个进账");
    }

    /// 三方账分桶：同键才算"两边都报"，同文件不同行不算。
    /// 这批数据必须自带非空的"仅快轨"与"两边都报"——现有 rust 语料快轨 0 条，
    /// 只靠它验不了这两桶（片A6 实测：桶逻辑的变异在那份语料上观察不到）。
    #[test]
    fn three_way_buckets_split_on_the_triple_key() {
        let items = vec![
            fnd(ENGINE_FAST, "a.rs", 1),
            fnd(ENGINE_MIR, "a.rs", 1),
            fnd(ENGINE_FAST, "b.rs", 7),
            fnd(ENGINE_MIR, "c.rs", 9),
            fnd(ENGINE_MIR, "a.rs", 2),
        ];
        let (both, only_fast, only_deep) = three_way_buckets(&items);
        assert_eq!(both, vec![key("a.rs", 1)], "同键的两条该进「两边都报」");
        assert_eq!(
            only_fast,
            vec![key("b.rs", 7)],
            "只有快轨报的该进「仅快轨」"
        );
        assert_eq!(
            only_deep,
            vec![key("c.rs", 9), key("a.rs", 2)],
            "仅深轨该含同文件的其他行"
        );
        // 键的第三个分量是行号，不是列：列不同不该把它们分开
        let same_line_diff_col = vec![
            {
                let mut f = fnd(ENGINE_FAST, "d.rs", 4);
                f.start_col = 0;
                f
            },
            {
                let mut f = fnd(ENGINE_MIR, "d.rs", 4);
                f.start_col = 12;
                f
            },
        ];
        let (both2, only_fast2, only_deep2) = three_way_buckets(&same_line_diff_col);
        assert_eq!(both2, vec![key("d.rs", 4)], "列差不该拆掉「两边都报」");
        assert!(only_fast2.is_empty() && only_deep2.is_empty());
        // 空输入 ⇒ 三桶全空（这就是老语料的形状，写出来免得后人以为它验过桶逻辑）
        let empty: Vec<Finding> = vec![];
        assert_eq!(three_way_buckets(&empty), (vec![], vec![], vec![]));
    }

    /// 规则不同则键不同：同一处两规则各报一条不该被并成"两边都报"。
    #[test]
    fn different_rules_do_not_merge_into_both() {
        let mut fast = fnd(ENGINE_FAST, "a.rs", 1);
        fast.rule = "RS-A".to_string();
        let mut deep = fnd(ENGINE_MIR, "a.rs", 1);
        deep.rule = "RS-B".to_string();
        let (both, only_fast, only_deep) = three_way_buckets(&[fast, deep]);
        assert!(both.is_empty(), "不同规则被并成了同一桶：{both:?}");
        assert_eq!(only_fast.len(), 1);
        assert_eq!(only_deep.len(), 1);
    }

    /// 档位解析：缺省 ast，`--engine` 三档认，未知档由调用方退出码 2 处理。
    #[test]
    fn engine_of_defaults_to_ast_and_reads_three_tiers() {
        assert_eq!(engine_of(&[]), Engine::Ast, "缺省档变了会改掉整个默认口径");
        assert_eq!(engine_of(&["--engine".into(), "mir".into()]), Engine::Mir);
        assert_eq!(engine_of(&["--engine".into(), "both".into()]), Engine::Both);
        assert_eq!(engine_of(&["--engine".into(), "ast".into()]), Engine::Ast);
    }
}
