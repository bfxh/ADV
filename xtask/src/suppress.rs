//! 抑制到期账门（M1 片3；蓝图 C2 延伸，RESEARCH D1 四要素）。
//!
//! 纪律：抑制必须有到期日——过期即红（刷新或删除），畸形即红（缺 reason/until）。
//! 防的是"永久豁免"：旧仓 noqa 无到期语义，抑制账只会腐烂。判据走真路径 =
//! 直接复用产品侧 `adv_rules::suppression`（同一解析器，两把尺不各说各话）。

use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

/// 对工作区成员的全部 Rust 源跑抑制到期账；返回违规清单（空 = 绿）。
pub fn run(root: &Path, today: &str) -> Result<Vec<String>> {
    let mut violations = Vec::new();
    for dir in crate::god::MEMBER_DIRS {
        let base = root.join(dir);
        if !base.is_dir() {
            anyhow::bail!("成员目录缺失：{dir}（MEMBER_DIRS 与 workspace members 失步）");
        }
        for entry in WalkDir::new(&base)
            .into_iter()
            .filter_entry(|e| walk_keep(e.file_name()))
            .filter_map(Result::ok)
        {
            if !is_rust_source(entry.file_type().is_file(), entry.path().extension()) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(root)
                .context("rel path")?
                .to_string_lossy()
                .replace('\\', "/");
            let src = fs_read(entry.path())?;
            violations.extend(file_violations(&rel, &src, today));
        }
    }
    Ok(violations)
}

/// 目录遍历保留判据（纯函数）：跳过 `target` 与点开头的名字。
/// 分诊发现这行的 `!=`/`&&`/`delete !` 三个突变存活（真跑里无人喂边界形状）⇒ 提取 + 真值表钉死。
pub fn walk_keep(file_name: &std::ffi::OsStr) -> bool {
    file_name != std::ffi::OsStr::new("target") && !file_name.to_string_lossy().starts_with('.')
}

/// 受扫文件判据（纯函数）：只扫 `.rs` 常规文件。
pub fn is_rust_source(is_file: bool, extension: Option<&std::ffi::OsStr>) -> bool {
    is_file && extension.is_some_and(|e| e == std::ffi::OsStr::new("rs"))
}

fn fs_read(p: &Path) -> Result<String> {
    std::fs::read_to_string(p).with_context(|| format!("读 {}", p.display()))
}

/// 单文件判定（纯函数，金丝雀直接喂源码）。
pub fn file_violations(file: &str, src: &str, today: &str) -> Vec<String> {
    let Ok(ast) = adv_parse::parse_source(adv_parse::Language::Rust, src) else {
        return vec![]; // 解析失败由 cargo build 兜底
    };
    let (ok, bad) = adv_rules::suppression::collect(&ast);
    let mut v = Vec::new();
    for s in ok {
        if !adv_rules::suppression::active(&s, today) {
            v.push(format!(
                "抑制到期 {file}:{} {}（until {}）——刷新日期或删除抑制",
                s.line, s.rule, s.until
            ));
        }
    }
    for m in bad {
        v.push(format!(
            "畸形抑制 {file}:{} {}——发现照报且须修复声明",
            m.line, m.why
        ));
    }
    v
}
