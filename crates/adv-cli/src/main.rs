//! ADV CLI 入口（M1 片1：`adv scan <目录>`；index/serve/gate 随里程碑落地）。
//!
//! 输出契约（C2/X15 裁定）：stdout 机器可读 JSONL（每行一条发现），stderr 人类摘要；
//! 退出码 0=扫描完成（发现的多少不改变退出码——报告与门禁是两回事，棘轮门另接）。

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => scan(&args[1..]),
        _ => {
            eprintln!("用法：adv scan <目录> [--rules <规则目录>]（默认 ./rules）");
            std::process::exit(2);
        }
    }
}

fn scan(args: &[String]) {
    let Some(target) = args.first() else {
        eprintln!("scan 需要一个目录参数");
        std::process::exit(2);
    };
    let rules_dir = args
        .windows(2)
        .find(|w| w[0] == "--rules")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from("rules"));
    // 作用域排除（M1 片1 真实仓裁定：vendor/ 第三方代码不归本仓规则管）——按路径组件匹配
    let excludes: Vec<String> = args
        .windows(2)
        .filter(|w| w[0] == "--exclude")
        .map(|w| w[1].clone())
        .collect();
    let rules = match adv_rules::load_rules(&rules_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("规则加载失败：{e}");
            std::process::exit(2);
        }
    };
    let mut files = 0usize;
    let mut findings_total = 0usize;
    let mut per_rule: Vec<(String, usize)> = Vec::new();
    for entry in ignore::Walk::new(target) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("遍历跳过：{e}");
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if excluded(path, &excludes) {
            continue;
        }
        let Some(lang) = language_of(path) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(path) else {
            continue; // 非 UTF-8/二进制：片1 跳过（账目见 FP 会计）
        };
        files += 1;
        let ast = match adv_parse::parse_source(lang, &src) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("解析失败 {}: {e}", path.display());
                continue;
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
        for (line, what) in adv_rules::suppression::expired(&ast, &today()) {
            eprintln!("抑制到期 {}:{} {what}（发现照报）", path.display(), line);
        }
        for f in adv_rules::run_matchers(&ast, path, lang, &rules, &today()) {
            println!("{}", serde_json::to_string(&f).unwrap_or_default());
            findings_total += 1;
            match per_rule.iter_mut().find(|(r, _)| r == &f.rule) {
                Some((_, n)) => *n += 1,
                None => per_rule.push((f.rule.clone(), 1)),
            }
        }
    }
    per_rule.sort();
    eprintln!("adv scan：{files} 个文件，{findings_total} 条发现");
    for (rule, n) in &per_rule {
        eprintln!("  {rule}: {n}");
    }
}

/// 今日日期（ISO）：`ADV_TODAY` 覆盖优先（判据时序独立性），否则系统 UTC。
fn today() -> String {
    if let Ok(d) = std::env::var("ADV_TODAY")
        && !d.is_empty()
    {
        return d;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil(secs / 86_400)
}

/// 天数 → YYYY-MM-DD（Howard Hinnant civil_from_days 算法，零依赖）。
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
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
