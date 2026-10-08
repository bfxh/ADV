//! RustSec advisory-db 的**读取面与同步面**（M3-3b）。
//!
//! 定位（M3-3a 先量定的，不是排期偷懒）：这一路做**覆盖核对**，不做第二把匹配尺——
//!
//! - RustSec 的 front matter **没有受影响区间**，只有 `[versions] patched = [...]`（修好的版本表）。
//!   要拿它当 matcher，得重实现 cargo-audit 的区间语义；而两库在 RUSTSEC ID 空间上实测**完全同步**
//!   （OSV 侧 1275 条 / RustSec DB 1274 条 / 交集 1274 / 仅 OSV 1 条 `RUSTSEC-2025-0000` / 仅 RustSec 0），
//!   增量价值在 OSV 独有的 1621 条非 RUSTSEC 条目（GHSA 等）——那部分归 M3-3a 的 matcher。
//! - 所以这里的产出是"**ID 级覆盖**"：两边条数、交集、以及两边**都有但字段不一致**的那些。
//!
//! 前置是外部 `git`（浅克隆实测 4s / 6.2MB）；不在 PATH ⇒ **判不了**（`Err`），不静默跳过。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// 上游仓库地址（浅克隆的源；协议是 https，凭据由外部 git 管）。
pub const REPO: &str = "https://github.com/RustSec/advisory-db";

/// 一条 RustSec 公告（只取对账要用的字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSecAdvisory {
    /// `RUSTSEC-YYYY-NNNN`。
    pub id: String,
    /// 包名（`[advisory].package`）。
    pub package: String,
    /// 发布日（原样带出）。
    pub date: String,
    /// `unsound` / `unmaintained` / `notice`；没有则 `None`（**不是漏洞通告**的标记）。
    pub informational: Option<String>,
    /// `[versions].patched`：修好的版本表（本层只对账、不据此判区间）。
    pub patched: Vec<String>,
    /// 别名（CVE/GHSA 互认）。
    pub aliases: Vec<String>,
}

/// 一份 DB 的读数。
#[derive(Debug)]
pub struct RustSecDb {
    /// 库目录。
    pub dir: PathBuf,
    /// 同步时钉下的 commit SHA（记账里读；没有就是 `None`）。
    pub sha: Option<String>,
    /// id → 公告。
    pub advisories: BTreeMap<String, RustSecAdvisory>,
}

/// 一次同步的读数。
#[derive(Debug, Default)]
pub struct RustSecSync {
    /// 这次跑完的 HEAD SHA。
    pub sha: String,
    /// 读到的公告条数。
    pub advisories: usize,
    /// 是新建克隆还是原地更新。
    pub fresh_clone: bool,
}

#[derive(Deserialize)]
struct FrontMatter {
    #[serde(default)]
    advisory: AdvisoryTable,
    #[serde(default)]
    versions: VersionsTable,
}

#[derive(Default, Deserialize)]
struct AdvisoryTable {
    #[serde(default)]
    id: String,
    #[serde(default)]
    package: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    informational: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Default, Deserialize)]
struct VersionsTable {
    #[serde(default)]
    patched: Vec<String>,
}

/// 解析一份 `RUSTSEC-*.md`：取**第一个** ```toml 围栏块当 front matter。
/// 形状不对（没有围栏 / TOML 解不开 / 没有 id）⇒ `Err`。
pub fn parse_advisory_md(text: &str) -> Result<RustSecAdvisory, String> {
    let start = text.find("```toml").ok_or("没有 ```toml 前置块")?;
    let rest = &text[start + "```toml".len()..];
    let end = rest.find("```").ok_or("前置块没有闭合的 ```")?;
    let fm: FrontMatter =
        toml::from_str(&rest[..end]).map_err(|e| format!("前置块不是合法 TOML：{e}"))?;
    if fm.advisory.id.trim().is_empty() {
        return Err("前置块没有 [advisory].id".to_string());
    }
    Ok(RustSecAdvisory {
        id: fm.advisory.id,
        package: fm.advisory.package,
        date: fm.advisory.date,
        informational: fm.advisory.informational.filter(|s| !s.trim().is_empty()),
        patched: fm.versions.patched,
        aliases: fm.advisory.aliases,
    })
}

fn walk_advdir(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).with_context(|| format!("列 {}", root.display()))? {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).with_context(|| format!("列 {}", dir.display()))? {
            let p = f?.path();
            let is_md = p.extension().and_then(|s| s.to_str()) == Some("md");
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if is_md && name.starts_with("RUSTSEC-") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 读一份本地 DB。**一份都读不出 ⇒ `Err`**；任何一份解不开 ⇒ `Err`（DB 损坏不许静默缩水覆盖）。
pub fn load_db(dir: &Path) -> Result<RustSecDb> {
    let crates = dir.join("crates");
    anyhow::ensure!(
        crates.is_dir(),
        "{} 里没有 crates/ 子目录（不是 advisory-db？）",
        dir.display()
    );
    let mut advisories = BTreeMap::new();
    for path in walk_advdir(&crates)? {
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("读 {}", path.display()))?;
        let adv = parse_advisory_md(&text).map_err(|e| anyhow!("{}：{e}", path.display()))?;
        advisories.insert(adv.id.clone(), adv);
    }
    anyhow::ensure!(
        !advisories.is_empty(),
        "{} 里一条 RUSTSEC-*.md 都没有——这不等于没有问题",
        dir.display()
    );
    let sha = crate::snapshot::load_manifest(dir.parent().unwrap_or(dir))
        .ok()
        .and_then(|m| m.rustsec)
        .map(|r| r.sha);
    Ok(RustSecDb {
        dir: dir.to_path_buf(),
        sha,
        advisories,
    })
}

fn git(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.args(args);
    let out = cmd
        .output()
        .map_err(|e| format!("起不了 git（{e}）——本层依赖外部 git，PATH 里没有就是判不了"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} 失败：{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 同步一份 DB 到 `dir`：已存在则原地 fetch + reset，否则浅克隆；随后钉 HEAD 的 SHA。
/// 返回值给调用方写进快照记账（`manifest.json` 的 `rustsec` 段）。
pub fn sync(dir: &Path) -> Result<RustSecSync> {
    let fresh_clone = !dir.join(".git").is_dir();
    if fresh_clone {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("建 {}", parent.display()))?;
        }
        git(
            None,
            &[
                "clone",
                "--quiet",
                "--depth",
                "1",
                REPO,
                &dir.to_string_lossy(),
            ],
        )
        .map_err(|e| anyhow!("克隆失败：{e}"))?;
    } else {
        git(Some(dir), &["fetch", "--quiet", "--depth", "1", "origin"])
            .map_err(|e| anyhow!("fetch 失败：{e}"))?;
        git(Some(dir), &["reset", "--hard", "--quiet", "FETCH_HEAD"])
            .map_err(|e| anyhow!("reset 失败：{e}"))?;
    }
    let sha = git(Some(dir), &["rev-parse", "HEAD"]).map_err(|e| anyhow!("取 HEAD 失败：{e}"))?;
    let advisories = walk_advdir(&dir.join("crates"))?.len();
    Ok(RustSecSync {
        sha,
        advisories,
        fresh_clone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"```toml
[advisory]
id = "RUSTSEC-2021-0120"
package = "abomonation"
date = "2021-10-17"
informational = "unsound"
aliases = ["CVE-2021-45708"]

[versions]
patched = []
```

# title
"#;

    #[test]
    fn front_matter_parses_into_the_four_fields_we_need() {
        let a = parse_advisory_md(SAMPLE).unwrap();
        assert_eq!(a.id, "RUSTSEC-2021-0120");
        assert_eq!(a.package, "abomonation");
        assert_eq!(a.informational.as_deref(), Some("unsound"));
        assert!(a.patched.is_empty());
        assert_eq!(a.aliases, vec!["CVE-2021-45708".to_string()]);
    }

    #[test]
    fn missing_fences_or_id_are_refused() {
        assert!(parse_advisory_md("# no front matter").is_err());
        assert!(parse_advisory_md("```toml\n[advisory]\npackage = \"x\"\n```\n").is_err());
    }

    #[test]
    fn informational_is_mapped_when_absent() {
        let a = parse_advisory_md(
            "```toml\n[advisory]\nid = \"RUSTSEC-2020-0001\"\npackage = \"p\"\ndate = \"\"\n[versions]\npatched = [\"1.0.0\"]\n```\n",
        )
        .unwrap();
        assert_eq!(a.informational, None);
        assert_eq!(a.patched, vec!["1.0.0".to_string()]);
    }
}
