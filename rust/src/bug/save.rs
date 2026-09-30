//! bug 子模块（S191）：存档区两条静态规则——**非原子写** / **读无版本门**。
//!
//! 判定档见 `spec/GAME-BUG-TAXONOMY.md` §一 街区 10（存档/序列化）。两条都只报
//! **线索**（结论与治理动作留给人），不判死：
//! - `save_nonatomic_write`：写点**直写存档路径**（`fs::write` / `File::create`），
//!   且目标不是临时/中间路径——写一半掉电/崩溃即档坏。本仓同族先例：账本末行截断的
//!   "坏账本像好账本"（spec/ROUNDLOG.md S187）。抑制口径**只看写点本身**，不做
//!   "函数里有没有 rename"的推断：实测那条推断只制造假阴性（原子写的落盘目标是
//!   scratch 路径，本就不含存档 token；"直写存档 + 无关 rename"却会被误静）；
//! - `save_load_no_version`：反序列化调用（`from_str`/`from_slice`/`deserialize`…）
//!   落在存档语料上，且**所在函数体**看不到版本门（version/schema/migrat/compat）
//!   ——更新后旧档被按新结构读（错位、丢档、静默重置）。
//!
//! 结构 vs 词法分工（与 `bug/rust.rs` 的字符级手写匹配同纪律）：
//! - **结构**（括号配平、函数体范围）在 `astscan::mask_rust` 掩码后的字符流上做
//!   ——字符串/注释被换成**等长**空格，字面量里的括号骗不到配平，且与原文逐字符
//!   对齐（掩码保长 ⇒ 下标可直接互用）；
//! - **词法**（存档/原子/版本 token）在**原文**上做——`"save.ron"` 与
//!   `with_extension("tmp")` 都在字面量里，掩码后不可见。
//!
//! 已知边界（本片刻意不做）：①`OpenOptions::new().create(true).truncate(true).open(p)`
//! 组合式打开未覆盖；②C#/GDScript 语言面未接（判定档队列第 5 项）；③只做
//! "函数体内看不到迹象"的抑制，不做跨函数数据流（那是污点域的事）。

use super::*;
use crate::astscan::{fn_re_search, mask_rust};

/// 存档语料 token（大小写不敏感子串）。`sav` 覆盖 save/saves/saved/SaveData/.sav——
/// 不用裸 `slot`：真实项目里 `ItemSlot`/`SLOT_SIZE` 是 UI 槽位（VoxelForge 实测）。
const SAVE_TOKENS: [&str; 2] = ["sav", "checkpoint"];
/// 临时/中间迹象（出现在**写点目标**文本里 ⇒ 那不是直写存档）
const TEMP_TOKENS: [&str; 5] = ["tmp", "temp", "atomic", "staging", "with_extension"];
/// 版本门/迁移迹象（出现在**所在函数体**里 ⇒ 读档处已有版本处理）
const VERSION_TOKENS: [&str; 4] = ["version", "schema", "migrat", "compat"];
/// 写锚点（`(` 允许空白间隔；`File::create_new` 由 `call_open` 的词边界自然区分）
const WRITE_ANCHORS: [&str; 3] = ["fs::write", "File::create", "File::create_new"];
/// 反序列化锚点（名字后允许 `::<T>` turbofish 再跟 `(`）
const DESER_NAMES: [&str; 6] =
    ["from_str", "from_slice", "from_reader", "from_bytes", "deserialize", "from_value"];

const NONATOMIC_MSG: &str =
    "存档写点非原子（直写目标路径，不是临时/中间文件）——写一半掉电/崩溃即档坏；\
先写临时文件 → sync_all → rename 覆盖（同族先例：本仓账本末行截断，坏账本曾被当成好账本）";
const NOVERSION_MSG: &str =
    "读存档未见过版本门/迁移分支（version/schema/migrat 皆无）——更新后旧档按新结构读\
（错位、丢档或静默重置）；加 format_version 字段与迁移分支";

/// 函数体行区间（1-based 闭区间）。
#[derive(Clone, Copy)]
struct Region {
    start: usize,
    end: usize,
}

/// 存档区两条规则的入口（只对 Rust 文件调用，见 bug.rs::scan_one 的分派）。
pub(crate) fn save_rules(src: &str, path: &str) -> Vec<Issue> {
    let cs: Vec<char> = src.chars().collect();
    let masked: Vec<char> = mask_rust(src).0; // 掩码保长 ⇒ 与 cs 逐字符对齐
    let starts = line_starts(&cs);
    let regions = fn_regions(&masked);
    let mut out = Vec::new();
    write_hits(&cs, &masked, &starts, path, &mut out);
    load_hits(&cs, &masked, &starts, path, &regions, &mut out);
    downgrade_tests(src, path, &mut out);
    out
}

// ---------- 写点：直写存档路径 ----------

fn write_hits(
    cs: &[char],
    masked: &[char],
    starts: &[usize],
    path: &str,
    out: &mut Vec<Issue>,
) {
    for anchor in WRITE_ANCHORS {
        let needle: Vec<char> = anchor.chars().collect();
        let mut from = 0usize;
        while let Some(pos) = find_from(cs, &needle, from) {
            from = pos + 1;
            if masked[pos] != needle[0] || !left_boundary(cs, pos) {
                continue; // 落在字符串/注释里，或不是独立调用名（`extend_from_slice` 非 `from_slice`）
            }
            let Some(open) = call_open(cs, pos + needle.len()) else { continue };
            let end = arg_end(masked, open, true);
            let arg = &cs[open + 1..end];
            if !has_token(arg, &SAVE_TOKENS) || has_token(arg, &TEMP_TOKENS) {
                continue;
            }
            out.push(issue(
                line_of(starts, pos),
                "save_nonatomic_write",
                NONATOMIC_MSG,
                path,
                "med",
            ));
        }
    }
}

// ---------- 读点：反序列化存档但无版本门 ----------

fn load_hits(
    cs: &[char],
    masked: &[char],
    starts: &[usize],
    path: &str,
    regions: &[Region],
    out: &mut Vec<Issue>,
) {
    for name in DESER_NAMES {
        let needle: Vec<char> = name.chars().collect();
        let mut from = 0usize;
        while let Some(pos) = find_from(cs, &needle, from) {
            from = pos + 1;
            if masked[pos] != needle[0] || !left_boundary(cs, pos) {
                continue;
            }
            let Some(open) = call_open(cs, pos + needle.len()) else { continue };
            let line = line_of(starts, pos);
            let mut evidence = cs[starts[line - 1]..end_of_line(cs, starts, line)].to_vec();
            evidence.extend_from_slice(&cs[open + 1..arg_end(masked, open, false)]);
            if !has_token(&evidence, &SAVE_TOKENS) {
                continue;
            }
            if region_has(cs, starts, regions, line, &VERSION_TOKENS) {
                continue;
            }
            out.push(issue(line, "save_load_no_version", NOVERSION_MSG, path, "low"));
        }
    }
}

// ---------- 公共小件 ----------

fn issue(line: usize, rule: &'static str, msg: &str, path: &str, sev: &'static str) -> Issue {
    Issue {
        line,
        rule,
        msg: msg.to_string(),
        file: path.to_string(),
        sev: Some(sev),
        kind: Some("clue"),
    }
}

/// 测试区降级（复用 `bug/rust.rs` 的三通道判定：tests 目录 / `*_test.rs` / `#[cfg(test)]` 行）。
fn downgrade_tests(src: &str, path: &str, out: &mut [Issue]) {
    let norm = path.replace('\\', "/").replace("_tmp/", "");
    let is_test_file = norm.ends_with("_test.rs") || contains_tests_segment(&norm);
    let cfg = if is_test_file { None } else { find_cfg_test_line(src) };
    if !is_test_file && cfg.is_none() {
        return;
    }
    for i in out.iter_mut() {
        if is_test_file || cfg.is_some_and(|t| i.line >= t) {
            i.sev = Some("low");
            i.kind = Some("clue");
            i.msg.push_str("（测试代码，降级）");
        }
    }
}

/// `name` 之后（允许空白）的 `(`；中间允许一层 `::<T>`（turbofish 内不含 `>`）。
/// 名字后紧跟词字符 ⇒ 不是这个调用（`from_str_radix` 不算 from_str）。
fn call_open(cs: &[char], name_end: usize) -> Option<usize> {
    if cs.get(name_end).is_some_and(|c| is_word(*c as u8)) {
        return None;
    }
    let mut j = name_end;
    while matches!(cs.get(j), Some(c) if c.is_whitespace()) {
        j += 1;
    }
    if cs.get(j) == Some(&'(') {
        return Some(j);
    }
    if !starts_with(cs, j, "::<") {
        return None;
    }
    let close = find_from(cs, &['>'], j + 3)?;
    let k = skip_ws(cs, close + 1);
    (cs.get(k) == Some(&'(')).then_some(k)
}

/// 参数区终点（char 下标，`(` 之后到配平的 `)` 或首个顶层 `,`）。
/// 配平在**掩码**字符流上做：字面量里的括号/逗号已被抹平。
fn arg_end(masked: &[char], open: usize, split_comma: bool) -> usize {
    let mut depth = 0i32;
    let mut j = open + 1;
    while let Some(&c) = masked.get(j) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return j;
                }
                depth -= 1;
            }
            ',' if split_comma && depth == 0 => return j,
            _ => {}
        }
        j += 1;
    }
    j
}

fn skip_ws(cs: &[char], mut i: usize) -> usize {
    while matches!(cs.get(i), Some(c) if c.is_whitespace()) {
        i += 1;
    }
    i
}

/// 左词边界：前一个字符不是词字符（`extend_from_slice` 里的 `from_slice` 不算调用名）。
/// 实测依据：`bench/s191_save_rules_probe.py` 首跑就把这一类列了出来。
fn left_boundary(cs: &[char], pos: usize) -> bool {
    pos == 0 || !matches!(cs.get(pos - 1), Some(c) if is_word(*c as u8))
}

fn starts_with(cs: &[char], at: usize, lit: &str) -> bool {
    lit.chars().enumerate().all(|(k, c)| cs.get(at + k) == Some(&c))
}

/// 子序列查找（无分配；字符口径，CJK 不会踩进多字节中间）。
fn find_from(cs: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || cs.len() < needle.len() {
        return None;
    }
    for i in from..=cs.len() - needle.len() {
        if cs[i..i + needle.len()] == *needle {
            return Some(i);
        }
    }
    None
}

/// 大小写不敏感子串（token 表口径）。
fn has_token(cs: &[char], toks: &[&str]) -> bool {
    let low: String = cs.iter().collect::<String>().to_lowercase();
    toks.iter().any(|t| low.contains(t))
}

/// 每行起点的 char 下标。
fn line_starts(cs: &[char]) -> Vec<usize> {
    let mut out = vec![0usize];
    for (i, c) in cs.iter().enumerate() {
        if *c == '\n' {
            out.push(i + 1);
        }
    }
    out
}

/// `pos` 所在行（1-based）。
fn line_of(starts: &[usize], pos: usize) -> usize {
    starts.partition_point(|&s| s <= pos)
}

/// 行尾（不含换行）char 下标。
fn end_of_line(cs: &[char], starts: &[usize], line: usize) -> usize {
    let next = starts.get(line).copied().unwrap_or(cs.len());
    let mut e = next;
    while e > 0 && matches!(cs.get(e - 1), Some('\n' | '\r')) {
        e -= 1;
    }
    e
}

/// 掩码字符流上的函数体区间（行号 1-based 闭区间；`{}` 配平，字符串/注释已抹平）。
fn fn_regions(masked: &[char]) -> Vec<Region> {
    let mut out = Vec::new();
    let mut stack: Vec<(usize, i64)> = Vec::new(); // (起始行, 进入时的深度)
    let mut depth = 0i64;
    for (idx, ln) in lines_of(masked).iter().enumerate() {
        let line = idx + 1;
        if ln.contains(&'{')
            && let Some(_name) = fn_re_search(&ln.iter().collect::<String>())
        {
            stack.push((line, depth));
        }
        let opens = ln.iter().filter(|c| **c == '{').count() as i64;
        let closes = ln.iter().filter(|c| **c == '}').count() as i64;
        depth += opens - closes;
        while let Some(&(start, d)) = stack.last() {
            if d >= depth {
                stack.pop();
                out.push(Region { start, end: line });
            } else {
                break;
            }
        }
    }
    for (start, _) in stack {
        out.push(Region { start, end: lines_of(masked).len() }); // 未闭合（截断文件）
    }
    out
}

fn lines_of(cs: &[char]) -> Vec<Vec<char>> {
    let mut out = vec![Vec::new()];
    for &c in cs {
        if c == '\n' {
            out.push(Vec::new());
        } else {
            out.last_mut().expect("至少一行").push(c);
        }
    }
    out
}

/// 命中行所在的**最内层**函数体里是否出现给定迹象；不在任何函数内 ⇒ 不抑制（无从判断）。
fn region_has(
    cs: &[char],
    starts: &[usize],
    regions: &[Region],
    line: usize,
    toks: &[&str],
) -> bool {
    let inner = regions
        .iter()
        .filter(|r| r.start <= line && line <= r.end)
        .max_by_key(|r| r.start);
    let Some(r) = inner else { return false };
    let from = starts[r.start - 1];
    let to = if r.end < starts.len() { starts[r.end] } else { cs.len() };
    has_token(&cs[from..to], toks)
}
