//! adv-secrets：secrets 检测——内化 Nosey Parker 的**薄封装**（M3-1）。
//!
//! 上游源码在 `third_party/noseyparker/`（出处锚见其 `VENDOR.md`，Apache-2.0，只读不改）。
//! 本 crate 只写"我们这一层"：输入路径 → 枚举文件 → 逐个内容寻址（`Blob::from_bytes`，
//! 同内容=同 id ⇒ 天然的按 blob 去重）→ 上游 `Matcher` 扫描 → 归一成我们的 [`SecretFinding`]。
//!
//! 为什么薄：上游 CLI 的 `cmd_scan.rs` 有 1255 行（参数解析、进度条、报告格式、github 枚举…），
//! 整搬等于把 CLI 也内化（依赖树 275 → 473 包）。我们只需要"扫一批文件出发现"这一条路径，
//! 其余（datastore 落盘/增量/report 形态）按后续切片逐步接。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use noseyparker::blob::Blob;
use noseyparker::blob_id_map::BlobIdMap;
use noseyparker::matcher::{Matcher, ScanResult};
use noseyparker::provenance::Provenance;
use noseyparker::provenance_set::ProvenanceSet;
use noseyparker::rules_database::RulesDatabase;

/// 单条 secrets 发现（归一后的形状；上游字段留在上游，接口只给这四样）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecretFinding {
    /// 上游规则码（如 `aws-access-key-id`）——不重命名，便于回上游对账。
    pub rule: String,
    /// 命中文件（用调用者给的路径形态；`/` 分隔）。
    pub path: String,
    /// 1 起。
    pub line: usize,
    /// 命中值掩码。X10 口径：定长 `***`，原文不出接口。
    pub masked: String,
}

/// 域版本（与工作区同源）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 单文件大小上限：超过就跳过（secrets 不该藏在一个几十 MB 的二进制里；跳过要留痕）。
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// 扫一批文件/目录，返回发现（按 路径、行号、规则码 排序）。
///
/// 内容寻址去重由上游 `Blob` 保证：**同内容的文件只扫一次**（`ScanResult::Seen*`），
/// 这正是"增量降噪"的机制点——本函数在同一次调用内生效；跨调用的持久增量要接 datastore（M3-1b）。
pub fn scan_paths(paths: &[PathBuf]) -> Result<Vec<SecretFinding>> {
    let syntaxes = noseyparker::defaults::get_builtin_rules().context("读上游内建规则集失败")?;
    let rules: Vec<noseyparker_rules::Rule> = syntaxes
        .iter_rules()
        .cloned()
        .map(noseyparker_rules::Rule::new)
        .collect();
    anyhow::ensure!(!rules.is_empty(), "上游内建规则集是空的（规则库读坏了吧）");
    let rules_db = RulesDatabase::from_rules(rules).context("编译规则库（vectorscan）失败")?;
    let seen_blobs: BlobIdMap<bool> = BlobIdMap::new();
    let mut matcher = Matcher::new(&rules_db, &seen_blobs, None).context("建 Matcher 失败")?;

    let mut out = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for path in paths {
        for file in enumerate_files(path)? {
            let meta =
                std::fs::metadata(&file).with_context(|| format!("看 {}", file.display()))?;
            if meta.len() > MAX_FILE_BYTES {
                skipped.push(format!("{}（{} 字节，超上限）", file.display(), meta.len()));
                continue;
            }
            let Ok(bytes) = std::fs::read(&file) else {
                skipped.push(format!("{}（读失败）", file.display()));
                continue;
            };
            scan_one(&mut matcher, &file, bytes, &mut out)
                .with_context(|| format!("扫 {}", file.display()))?;
        }
    }
    if !skipped.is_empty() {
        // "没扫到"不许长得像"干净"（本仓同一条纪律）：跳过必须留痕。
        eprintln!("adv-secrets：跳过 {} 个文件：", skipped.len());
        for s in &skipped {
            eprintln!("  - {s}");
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn enumerate_skip(dir: &Path) -> bool {
    matches!(
        dir.file_name().and_then(|s| s.to_str()),
        Some(".git" | "target" | "node_modules" | "__pycache__")
    )
}

fn enumerate_files(root: &Path) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    anyhow::ensure!(root.is_dir(), "路径不存在：{}", root.display());
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && enumerate_skip(e.path())))
    {
        let entry = entry.with_context(|| format!("遍历 {}", root.display()))?;
        if entry.file_type().is_file() {
            out.push(entry.path().to_path_buf());
        }
    }
    Ok(out)
}

fn scan_one(
    matcher: &mut Matcher<'_>,
    file: &Path,
    bytes: Vec<u8>,
    out: &mut Vec<SecretFinding>,
) -> Result<()> {
    let blob = Blob::from_bytes(bytes);
    let prov = ProvenanceSet::single(Provenance::from_file(file.to_path_buf()));
    let ScanResult::New(matches) = matcher.scan_blob(&blob, &prov)? else {
        // Seen*：同内容见过 ⇒ 不重复出账（去重在这里，不在调用方）。
        return Ok(());
    };
    for m in matches {
        let start = m.matching_input_offset_span.start;
        let line = 1 + blob.bytes[..start.min(blob.bytes.len())]
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        out.push(SecretFinding {
            rule: m.rule.id().to_string(),
            path: file.to_string_lossy().replace('\\', "/"),
            line,
            // X10：定长 `***`。上游的"部分掩码 40 bits 熵保底"提案档留 M3-1b 再对齐。
            masked: "***".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_core() {
        assert_eq!(super::version(), adv_core::version());
    }
}
