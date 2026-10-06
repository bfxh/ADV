//! god 门（蓝图 C2）：文件/函数/类型成员三轴，硬阈 + 棘轮基线（只准减）+ 金丝雀自证。
//!
//! 与旧仓 god_gate.py 的关系：思想原样移植（棘轮两级制 = 基线禁恶化 + 硬阈禁历史债），
//! 补上旧仓缺的**文件级硬阈**（RESEARCH 00 §②：382 条祖父化教训）；载体从外挂脚本
//! 改为 xtask 内建（RESEARCH D4：生态空白确认自研）。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use walkdir::WalkDir;

/// 文件行数硬阈（旧仓缺、本次补上：RESEARCH 00）。
pub const MAX_FILE_LINES: u64 = 800;
/// 函数行数硬阈。
pub const MAX_FN_LINES: u64 = 120;
/// 类型成员数硬阈（impl 块/结构体字段/枚举变体）。
pub const MAX_TYPE_MEMBERS: u64 = 24;

/// 基线文件位置（相对工作区根）。
pub const BASELINE_PATH: &str = "tools/baselines/god-baseline.json";

#[derive(Serialize, Deserialize)]
struct Baseline {
    tool: String,
    entries: BTreeMap<String, u64>,
}

/// 成员目录：新 crate 落地即受门管（无需登记）。
pub const MEMBER_DIRS: &[&str] = &[
    "crates/adv-core",
    "crates/adv-parse",
    "crates/adv-rules",
    "crates/adv-taint",
    "crates/adv-secrets",
    "crates/adv-sca",
    "crates/adv-index",
    "crates/adv-server",
    "crates/adv-sandbox",
    "crates/adv-bin",
    "crates/adv-cli",
    "crates/adv-ast-rust",
    "xtask",
];

/// 对单份 Rust 源做三轴计量（纯函数，金丝雀测试直接喂内存源码）。
pub fn analyze_source(rel: &str, src: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    out.insert(format!("file:{rel}"), src.lines().count() as u64);
    let file = match syn::parse_file(src) {
        Ok(f) => f,
        Err(_) => return out, // 解析失败交由 cargo build 兜底，门不重复报
    };
    let mut c = Counter {
        rel: rel.to_string(),
        ctx: Vec::new(),
        in_fn: false,
        out: &mut out,
    };
    c.visit_file(&file);
    out
}

struct Counter<'a> {
    rel: String,
    ctx: Vec<String>,
    in_fn: bool,
    out: &'a mut BTreeMap<String, u64>,
}

impl<'a> Counter<'a> {
    fn fn_key(&self, name: &str) -> String {
        if self.ctx.is_empty() {
            format!("fn:{}::{name}", self.rel)
        } else {
            format!("fn:{}::{}::{name}", self.rel, self.ctx.join("::"))
        }
    }

    fn record_fn(&mut self, name: &str, start: usize, end: usize) {
        let lines = end.saturating_sub(start) as u64 + 1;
        self.out.insert(self.fn_key(name), lines);
    }
}

fn type_name_of(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "impl".to_string()),
        syn::Type::Reference(r) => type_name_of(&r.elem),
        _ => "impl".to_string(),
    }
}

impl<'ast, 'a> Visit<'ast> for Counter<'a> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        if self.in_fn {
            return; // 嵌套 fn 不单独计账（与旧门同口径）
        }
        self.in_fn = true;
        self.record_fn(
            &i.sig.ident.to_string(),
            i.sig.span().start().line,
            i.block.span().end().line,
        );
        self.in_fn = false;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        if self.in_fn {
            return;
        }
        self.in_fn = true;
        self.record_fn(
            &i.sig.ident.to_string(),
            i.sig.span().start().line,
            i.block.span().end().line,
        );
        self.in_fn = false;
    }

    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        if let Some(block) = &i.default
            && !self.in_fn
        {
            self.in_fn = true;
            self.record_fn(
                &i.sig.ident.to_string(),
                i.sig.span().start().line,
                block.span().end().line,
            );
            self.in_fn = false;
        }
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if self.in_fn {
            return;
        }
        let name = type_name_of(&i.self_ty);
        self.out.insert(
            format!("type:{}::{}(impl)", self.rel, name),
            i.items.len() as u64,
        );
        self.ctx.push(name);
        syn::visit::visit_item_impl(self, i);
        self.ctx.pop();
    }

    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        let name = i.ident.to_string();
        let n = match &i.fields {
            syn::Fields::Named(f) => f.named.len(),
            syn::Fields::Unnamed(f) => f.unnamed.len(),
            syn::Fields::Unit => 0,
        };
        self.out
            .insert(format!("type:{}::{}(fields)", self.rel, name), n as u64);
    }

    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        self.out.insert(
            format!("type:{}::{}(variants)", self.rel, i.ident),
            i.variants.len() as u64,
        );
    }

    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        self.out.insert(
            format!("type:{}::{}(items)", self.rel, i.ident),
            i.items.len() as u64,
        );
    }
}

/// 收集全部成员 crate 的计量（只扫工作区成员——旧仓 Rust 引擎不在成员内，不受新门管）。
pub fn collect_workspace_entries(root: &Path) -> Result<BTreeMap<String, u64>> {
    let mut entries = BTreeMap::new();
    // 试验面（spec/maturity.json 登记）不计量：docs/DESIGN-DEBT-GATE.md §3。
    // 给合成夹具记规模棘轮，等于把维护形的税交给测试材料——只拖慢，不出正确性。
    let experimental = crate::maturity::experimental_patterns(root);
    for dir in MEMBER_DIRS {
        let base = root.join(dir);
        if !base.is_dir() {
            anyhow::bail!("成员目录缺失：{dir}（MEMBER_DIRS 与 workspace members 失步）");
        }
        for entry in WalkDir::new(&base)
            .into_iter()
            .filter_entry(|e| {
                e.file_name() != "target" && !e.file_name().to_string_lossy().starts_with('.')
            })
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "rs") {
                let rel = entry
                    .path()
                    .strip_prefix(root)
                    .context("rel path")?
                    .to_string_lossy()
                    .replace('\\', "/");
                if experimental
                    .iter()
                    .any(|p| crate::maturity::pattern_matches(p, &rel))
                {
                    continue; // 试验面不计量（登记见 spec/maturity.json）
                }
                let src = fs::read_to_string(entry.path())
                    .with_context(|| format!("read {}", entry.path().display()))?;

                for (k, v) in analyze_source(&rel, &src) {
                    entries.insert(k, v);
                }
            }
        }
    }
    Ok(entries)
}

fn hard_threshold(key: &str) -> Option<u64> {
    if key.starts_with("file:") {
        Some(MAX_FILE_LINES)
    } else if key.starts_with("fn:") {
        Some(MAX_FN_LINES)
    } else if key.starts_with("type:") {
        Some(MAX_TYPE_MEMBERS)
    } else {
        None
    }
}

/// 硬阈违规（禁历史债：任何基线都救不回）。
pub fn hard_violations(entries: &BTreeMap<String, u64>) -> Vec<String> {
    let mut v = Vec::new();
    for (k, &val) in entries {
        if let Some(cap) = hard_threshold(k)
            && val > cap
        {
            v.push(format!("硬阈 {k} = {val} > {cap}"));
        }
    }
    v
}

/// 棘轮违规（基线禁恶化；新键 = 未登记面，须 `--write` 登记并披露）。
pub fn ratchet_violations(
    entries: &BTreeMap<String, u64>,
    baseline: &BTreeMap<String, u64>,
) -> Vec<String> {
    let mut v = Vec::new();
    for (k, &val) in entries {
        match baseline.get(k) {
            Some(&base) if val > base => v.push(format!("棘轮 {k} = {val} > 基线 {base}")),
            None => v.push(format!("未登记 {k} = {val}（新面：--write 登记并披露）")),
            _ => {}
        }
    }
    v
}

/// 写基线：先过硬阈再落盘（旧仓教训：`--write-baseline` 会吞掉新超标面 ⇒ 先拆到阈内再记）。
pub fn write_baseline(root: &Path, entries: &BTreeMap<String, u64>) -> Result<()> {
    let hard = hard_violations(entries);
    if !hard.is_empty() {
        anyhow::bail!(
            "存在硬阈违规，拒绝写基线（先拆到阈内再登记）：\n{}",
            hard.join("\n")
        );
    }
    let path = root.join(BASELINE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let baseline = Baseline {
        tool: "xtask god v0".to_string(),
        entries: entries.clone(),
    };
    fs::write(&path, serde_json::to_string_pretty(&baseline)?)?;
    println!("基线已写 {}（{} 项）", BASELINE_PATH, entries.len());
    Ok(())
}

/// 执行 god 门：返回违规清单（空 = 绿）。
pub fn run(root: &Path) -> Result<Vec<String>> {
    let entries = collect_workspace_entries(root)?;
    let path = root.join(BASELINE_PATH);
    let baseline: Baseline = serde_json::from_str(
        &fs::read_to_string(&path).with_context(|| format!("读 {}", path.display()))?,
    )?;
    let mut violations = hard_violations(&entries);
    violations.extend(ratchet_violations(&entries, &baseline.entries));
    Ok(violations)
}

/// 初始化基线（`--write`）：当前状态必须过硬阈，然后落盘。
pub fn init_or_write(root: &Path) -> Result<()> {
    let entries = collect_workspace_entries(root)?;
    write_baseline(root, &entries)
}
