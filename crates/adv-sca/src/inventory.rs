//! 清单段（Inventory）：把 `Cargo.lock` 与被扫仓的 `Cargo.toml` 读成**可比对的两份账**。
//!
//! 分工照 Syft→Grype：这一段只出事实、不判定（判定在 [`crate::lockcheck`]，
//! 漏洞匹配留给 M3-3 的 matcher）。两条理由：清单错了匹配再准也没用；以及
//! "解析失败"必须是**判不了**而不是"读出一张空清单"（空清单会长得像干净）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// 锁里的一条包记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// 包名（锁里 `name`）。
    pub name: String,
    /// 锁定的版本号；v1/v2 时代的内联写法可能为空串。
    pub version: String,
    /// `None` ⇒ 本地包（路径依赖，锁里不记 source）；`Some` ⇒ registry/git 来源。
    pub source: Option<String>,
    /// registry 包的 `checksum`。本地包天然没有。
    pub checksum: Option<String>,
    /// 锁里 `dependencies` 的**包名**：条目形如 `syn`、`syn 2.0.104`、
    /// `winapi 0.3.9 (registry+https://github.com/rust-lang/crates.io-index)`，取首 token。
    pub dependencies: Vec<String>,
}

impl Package {
    /// 本地包（路径依赖）：锁里没有 `source`。
    pub fn is_local(&self) -> bool {
        self.source.is_none()
    }
}

/// 一把锁读出来的事实。
#[derive(Debug)]
pub struct Inventory {
    /// 锁文件路径（原样，含调用方给的形态）。
    pub lock: PathBuf,
    /// 锁自带的 `version` 字段（v3 及更早没有）。
    pub format_version: Option<u64>,
    /// 全部 `[[package]]` 记录，按锁里的顺序。
    pub packages: Vec<Package>,
}

impl Inventory {
    /// 按名找**本地**包（同名多版本时取第一条，由 LC-3 另行出信号）。
    pub fn find_local(&self, name: &str) -> Option<&Package> {
        self.packages
            .iter()
            .find(|p| p.is_local() && p.name == name)
    }
}

/// 工作区成员：清单声明的那份账。
#[derive(Debug)]
pub struct Member {
    /// 成员包名（`[package].name`）。
    pub name: String,
    /// 该成员清单的路径。
    pub manifest: PathBuf,
    /// 该清单四个段位里声明的依赖**真名**（rename 取 `package = "…"`）。
    pub declared: BTreeSet<String>,
}

/// 一把锁旁边的世界长什么样（成员集合决定 LC-2 的比对范围）。
#[derive(Debug)]
pub struct Workspace {
    /// 根清单所在目录（锁的同级目录）。
    pub root: PathBuf,
    /// 展开 `[workspace].members`（含 glob、减 `exclude`）后的成员；单体 crate 就是它自己。
    pub members: Vec<Member>,
}

#[derive(Deserialize)]
struct LockFile {
    #[serde(default)]
    version: Option<u64>,
    #[serde(default)]
    package: Vec<RawPackage>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// 解析锁文本。**fail-closed**：读不出 `[[package]]` 就是判不了（`Err`），不给空清单。
pub fn parse_lock(text: &str, lock: &Path) -> Result<Inventory> {
    let parsed: LockFile =
        toml::from_str(text).with_context(|| format!("解析锁 {}", lock.display()))?;
    if parsed.package.is_empty() {
        return Err(anyhow!(
            "{} 里一条 [[package]] 都没有：这不是能据以判定的清单（空清单≠干净）",
            lock.display()
        ));
    }
    Ok(Inventory {
        lock: lock.to_path_buf(),
        format_version: parsed.version,
        packages: parsed
            .package
            .into_iter()
            .map(|p| Package {
                name: p.name,
                version: p.version,
                source: p.source,
                checksum: p.checksum,
                dependencies: p
                    .dependencies
                    .iter()
                    .map(|d| dep_name(d).to_string())
                    .collect(),
            })
            .collect(),
    })
}

fn dep_name(entry: &str) -> &str {
    entry.split(' ').next().unwrap_or(entry)
}

/// 从磁盘读一把锁并解析（读不出内容 ⇒ `Err`，不返回空清单）。
pub fn load_lock(lock: &Path) -> Result<Inventory> {
    let text = std::fs::read_to_string(lock).with_context(|| format!("读 {}", lock.display()))?;
    parse_lock(&text, lock)
}

/// 读锁所在目录的工作区成员清单（无 `[workspace]` 的单体 crate 也算一个成员）。
pub fn load_workspace(root: &Path) -> Result<Workspace> {
    let manifest = root.join("Cargo.toml");
    let text =
        std::fs::read_to_string(&manifest).with_context(|| format!("读 {}", manifest.display()))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("解析 {}", manifest.display()))?;

    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(ws) = table.get("workspace").and_then(|w| w.as_table()) {
        for entry in ws
            .get("members")
            .and_then(|m| m.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            if let Some(s) = entry.as_str() {
                paths.extend(expand_member(root, s));
            }
        }
        for skip in ws
            .get("exclude")
            .and_then(|m| m.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            if let Some(s) = skip.as_str() {
                let p = normalize(s);
                paths.retain(|m| !(m == &p || m.starts_with(&p)));
            }
        }
    }
    if table.get("package").and_then(|p| p.get("name")).is_some() {
        paths.push(PathBuf::from("."));
    }
    anyhow::ensure!(
        !paths.is_empty(),
        "{} 旁读不出任何成员（既没有 [[package]] 也没有 [workspace].members）",
        root.display()
    );

    let mut members = Vec::new();
    for rel in paths {
        let dir = if rel == Path::new(".") {
            root.to_path_buf()
        } else {
            root.join(&rel)
        };
        let path = dir.join("Cargo.toml");
        let mtext =
            std::fs::read_to_string(&path).with_context(|| format!("读 {}", path.display()))?;
        let mtable: toml::Table =
            toml::from_str(&mtext).with_context(|| format!("解析 {}", path.display()))?;
        let name = mtable
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .with_context(|| format!("{} 没有 package.name", path.display()))?
            .to_string();
        members.push(Member {
            name,
            manifest: path,
            declared: declared_deps(&mtable),
        });
    }
    Ok(Workspace {
        root: root.to_path_buf(),
        members,
    })
}

/// `crates/*` 展成该目录下的直接子目录；其余按字面路径。
fn expand_member(root: &Path, entry: &str) -> Vec<PathBuf> {
    match entry.strip_suffix("/*") {
        Some(raw_prefix) => {
            // 目录条目补一个尾斜杠再拼文件名：`crates/*` → `crates/adv-core`，
            // 少了这个斜杠就会拼成 `cratesadv-core`（写完当场用真成员列表验的）。
            let prefix = format!(
                "{}/",
                normalize(raw_prefix).to_string_lossy().replace('\\', "/")
            );
            let dir = root.join(&prefix);
            let mut out = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if e.path().join("Cargo.toml").is_file() {
                        out.push(PathBuf::from(format!(
                            "{prefix}{}",
                            e.file_name().to_string_lossy()
                        )));
                    }
                }
            }
            out.sort();
            out
        }
        None => vec![normalize(entry)],
    }
}

/// cargo 的条目一律用 `/`；Windows 上把 `\` 也认下来。
fn normalize(entry: &str) -> PathBuf {
    PathBuf::from(entry.replace('\\', "/"))
}

const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// 一份清单声明了哪些依赖真名。四段：普通/dev/build/target 段下的同样三段。
/// `foo = { package = "bar" }` 记 `bar`（锁里出现的是真名，不解析重命名就会假红）。
pub fn declared_deps(table: &toml::Table) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for section in SECTIONS {
        if let Some(t) = table.get(section).and_then(|d| d.as_table()) {
            collect(&mut out, t);
        }
    }
    if let Some(targets) = table.get("target").and_then(|t| t.as_table()) {
        for cfg in targets.values().filter_map(|v| v.as_table()) {
            for section in SECTIONS {
                if let Some(t) = cfg.get(section).and_then(|d| d.as_table()) {
                    collect(&mut out, t);
                }
            }
        }
    }
    out
}

fn collect(out: &mut BTreeSet<String>, deps: &toml::Table) {
    for (alias, value) in deps {
        let real = match value {
            toml::Value::Table(t) => t
                .get("package")
                .and_then(|p| p.as_str())
                .unwrap_or(alias)
                .to_string(),
            toml::Value::String(_) | toml::Value::Array(_) => alias.to_string(),
            _ => continue,
        };
        out.insert(real);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = r#"
version = 4

[[package]]
name = "app"
version = "0.1.0"
dependencies = ["syn 2.0.104 (registry+https://github.com/rust-lang/crates.io-index)", "libc"]

[[package]]
name = "syn"
version = "2.0.104"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0a5ccc9"
"#;

    #[test]
    fn dependency_entries_are_cut_to_the_package_name() {
        let inv = parse_lock(TWO, Path::new("Cargo.lock")).unwrap();
        let app = inv.find_local("app").unwrap();
        assert_eq!(
            app.dependencies,
            vec!["syn".to_string(), "libc".to_string()]
        );
        assert_eq!(inv.format_version, Some(4));
        assert_eq!(inv.packages.len(), 2);
    }

    #[test]
    fn lock_without_packages_is_refused_not_reported_empty() {
        let err = parse_lock("version = 4\n", Path::new("Cargo.lock")).unwrap_err();
        assert!(err.to_string().contains("[[package]]"), "{}", err);
    }

    #[test]
    fn rename_uses_the_real_package_name() {
        let t: toml::Table = toml::from_str(
            "[dependencies]\nb = { package = \"bar\", version = \"1\" }\nplain = \"1\"\n",
        )
        .unwrap();
        let d = declared_deps(&t);
        assert!(d.contains("bar"), "{d:?}");
        assert!(d.contains("plain"), "{d:?}");
        assert!(!d.contains("b"), "重命名别名不该当依赖名用：{d:?}");
    }

    #[test]
    fn dev_and_target_sections_count() {
        let t: toml::Table = toml::from_str(
            "[dependencies]\na = \"1\"\n[dev-dependencies]\nd = \"1\"\n\
             [target.'cfg(windows)'.dependencies]\nw = \"1\"\n",
        )
        .unwrap();
        let d = declared_deps(&t);
        assert_eq!(d.iter().cloned().collect::<Vec<_>>(), vec!["a", "d", "w"]);
    }
}
