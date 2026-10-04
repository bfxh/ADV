//! 版本/tag 锁步门（蓝图 C3）：版本常量必须与 git tag 对账，断档即红。
//!
//! 旧仓教训（RESEARCH 00 §②）：SERVER_VERSION 2.94.0 vs 最新 tag v2.58.0，
//! 36 个 minor 未打 tag ⇒ 期间零发布。此门把该事故变成不可重现的结构约束。
//!
//! 语义：
//! - 无 `v*` tag（引导期）：仅允许 `0.1.0`（骨架版本常量），其余一律红；
//! - 有 tag：workspace 版本必须精确等于最新 `v*` tag（允许 tag 滞后版本 ⇒ 打 tag）。

/// 解析 `X.Y.Z` 三段版本。
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.trim().split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    it.next().is_none().then_some((a, b, c))
}

/// 从 tag 集合提取语义版本（`v` 前缀可选；非三段数字的 tag 忽略）。
/// 新项目 tag 命名空间：`adv-vX.Y.Z`。
/// 2026-10-04 首跑裁定：新项目与旧仓共享 git 历史，旧 `v*` tag（v2.58.0 等）会污染
/// 锁步判定 ⇒ 新 tag 一律带 `adv-v` 前缀，门只认该前缀（历史隔离，不改写旧 tag）。
pub const TAG_PREFIX: &str = "adv-v";

/// 从 tag 集合提取语义版本（只认 `adv-v` 前缀；非三段数字的忽略）。
pub fn versions_from_tags(tags: &[String]) -> Vec<(u64, u64, u64)> {
    tags.iter()
        .filter_map(|t| t.strip_prefix(TAG_PREFIX))
        .filter_map(parse_version)
        .collect()
}

/// 纯函数裁决（金丝雀测试直接喂合成输入）。
pub fn check_lockstep(version: &str, tags: &[String]) -> Result<(), String> {
    let ver = parse_version(version).ok_or_else(|| format!("版本号非法：{version}"))?;
    let mut tagged = versions_from_tags(tags);
    if tagged.is_empty() {
        if ver == (0, 1, 0) {
            return Ok(());
        }
        return Err(format!(
            "无 {TAG_PREFIX} tag 且版本 {version} != 0.1.0：引导期只许 0.1.0，发版请先打 tag"
        ));
    }
    tagged.sort();
    let latest = tagged[tagged.len() - 1];
    if latest != ver {
        return Err(format!(
            "锁步断档：workspace 版本 {version} != 最新 tag {TAG_PREFIX}{}.{}.{}（旧仓 36 minor 断档教训）",
            latest.0, latest.1, latest.2
        ));
    }
    Ok(())
}

/// 从工作区根 Cargo.toml 读 `[workspace.package]` version。
pub fn workspace_version(root: &std::path::Path) -> anyhow::Result<String> {
    let raw = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let t: toml::Table = toml::from_str(&raw)?;
    t.get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml 缺 workspace.package.version"))
}

/// 收集 `adv-v*` tag（新项目命名空间；旧仓 `v*` tag 不参与判定）。
pub fn git_tags(root: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let out = std::process::Command::new("git")
        .args(["tag", "--list", "adv-v*"])
        .current_dir(root)
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// 执行锁步门：绿 = Ok(())，红 = Err(原因)。
pub fn run(root: &std::path::Path) -> anyhow::Result<()> {
    let version = workspace_version(root)?;
    let tags = git_tags(root)?;
    check_lockstep(&version, &tags).map_err(anyhow::Error::msg)
}
