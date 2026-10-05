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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => scan(&args[1..]),
        _ => {
            eprintln!(
                "用法：adv scan <目录> [--rules <规则目录>] [--engine ast|mir|both] [--driver <路径>]"
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
        if engine != Engine::Mir {
            scan_fast(path, lang, &src, &rules, &mut tally);
        }
        if engine != Engine::Ast {
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
            tally.deep_failed += 1;
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
    if engine != Engine::Ast && tally.deep_failed > 0 {
        eprintln!(
            "adv：深轨有 {} 个文件未跑成，退出码 2（不折算为无发现）",
            tally.deep_failed
        );
        std::process::exit(2);
    }
}

/// 三方账：两边都报 / 仅快轨 / 仅深轨。键 = (规则, 文件, 起始行)；列不参与
/// （两引擎给的列粒度口径不同：快轨是 AST span，深轨是 MIR terminator span）。
fn three_way(items: &[Finding]) {
    let key = |f: &Finding| (f.rule.clone(), f.file.clone(), f.start_line);
    let fast: Vec<_> = items
        .iter()
        .filter(|f| f.engine == adv_rules::matcher::ENGINE_FAST)
        .map(key)
        .collect();
    let deep: Vec<_> = items
        .iter()
        .filter(|f| f.engine == adv_rules::matcher::ENGINE_MIR)
        .map(key)
        .collect();
    let both: Vec<_> = fast.iter().filter(|k| deep.contains(k)).cloned().collect();
    let only_fast: Vec<_> = fast.iter().filter(|k| !both.contains(k)).cloned().collect();
    let only_deep: Vec<_> = deep.iter().filter(|k| !both.contains(k)).cloned().collect();
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
