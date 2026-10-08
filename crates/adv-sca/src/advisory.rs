//! OSV advisory 的模型与「版本是否落在受影响区间」的判定。
//!
//! 形状按**真数据**锚定（`tests/data/osv/` 里是两条真 advisory 的逐字切片，出处写在 README）：
//!
//! - `affected[].ranges[].type == "SEMVER"`，`events` 是**交替出现的多区间**：
//!   `[{"introduced":"0.0.0-0"},{"fixed":"0.2.0"},{"introduced":"0.2.1-0"}, …]`
//!   （RUSTSEC-2020-0071 实测 16 个事件 = 8 段）；也有 `[{"introduced":"0"},{"last_affected":"0.3.24"}]`
//!   这种两事件闭区间（GHSA-2226-4v3c-cff8）。
//! - 上界语义：`fixed` 是**开**（修好的版本不再受影响），`last_affected` 是**闭**（它本身仍受影响）。
//! - 哨兵：`introduced: "0"` 是「从头」，`"0.0.0-0"` 是 semver 意义上**小于 `0.0.0`** 的占位。
//! - `affected[].versions` 是显式版本表（两条真语料里都为空）——非空时按它判，忽略 ranges。
//! - 严重度有两个出处：`severity[].score`（CVSS 向量，RustSec 侧）与
//!   `database_specific.severity`（GHSA 侧的 `MODERATE` 这类词）；`database_specific.informational`
//!   非空表示「不是漏洞通告」（RustSec 的 unmaintained 类），本层如实带出，不做降级判断。
//!
//! 判定失败一律 `Err`（版本串解析不了 ⇒ 判不了），不返回 `false` —— 把「解析不了」读成「不受影响」
//! 正是漏报的入口。

use semver::Version;
use serde::Deserialize;
use serde_json::Value;

/// 一条 OSV 记录（只留判定与报账要用的键；`details` 这类长文本刻意不进模型）。
#[derive(Debug, Clone, Deserialize)]
pub struct OsvRecord {
    /// advisory 号（如 `RUSTSEC-2020-0071`）。
    pub id: String,
    /// 同一漏洞的其它库编号（CVE/GHSA 互认靠它）。
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 一行摘要。
    #[serde(default)]
    pub summary: String,
    /// 上游最后修改时间（原样带出，不解析）。
    #[serde(default)]
    pub modified: String,
    /// 受影响条目（按包 + 区间/版本表）。
    #[serde(default)]
    pub affected: Vec<Affected>,
    /// CVSS 向量表（GHSA 侧常为 null）。
    #[serde(default)]
    pub severity: Vec<Severity>,
    /// 自由字段（`severity` 词、`informational`、`cvss`、`source` 都可能在这里）。
    #[serde(default)]
    pub database_specific: Option<Value>,
}

/// 一条 CVSS 向量。
#[derive(Debug, Clone, Deserialize)]
pub struct Severity {
    /// 向量类型（`CVSS_V3` 等）。
    #[serde(rename = "type")]
    pub kind: String,
    /// 向量本体（`CVSS:3.1/…`）。
    pub score: String,
}

/// 某个包在某条 advisory 下的受影响范围。
#[derive(Debug, Clone, Deserialize)]
pub struct Affected {
    /// 包标识（生态 + 名）。
    pub package: OsvPackage,
    /// 区间表（`SEMVER` 才参与判定）。
    #[serde(default)]
    pub ranges: Vec<OsvRange>,
    /// 显式受影响版本表；非空时以它为准。
    #[serde(default)]
    pub versions: Vec<String>,
    /// 自由字段。**`informational` 就住在这一层**（实测取值 `unsound`/`unmaintained`/`notice`），
    /// 顶层那个 `database_specific` 只有 license/cwe 这类——读错层会把 unmaintained 当漏洞
    /// 判红（全库 496 条），是这次实测才发现的漏读。
    #[serde(default)]
    pub database_specific: Option<Value>,
}

/// OSV 里的包标识。
#[derive(Debug, Clone, Deserialize)]
pub struct OsvPackage {
    /// 包名（crates.io 上就是 crate 名）。
    pub name: String,
    /// 生态（本层只认 `crates.io`）。
    pub ecosystem: String,
}

/// 一段区间描述（事件序列）。
#[derive(Debug, Clone, Deserialize)]
pub struct OsvRange {
    /// 区间类型（`SEMVER` / `ECOSYSTEM` / `GIT`）。
    #[serde(rename = "type")]
    pub kind: String,
    /// 事件序列（交替的 introduced 与 fixed/last_affected）。
    #[serde(default)]
    pub events: Vec<OsvEvent>,
}

/// 一个事件只带三种键之一；`limit` 是 ECOSYSTEM 档用的，见了要**忽略得响亮**
/// （本层只认 SEMVER，非 SEMVER 的区间整段不参与判定）。
#[derive(Debug, Clone, Deserialize)]
pub struct OsvEvent {
    /// 区间起点；`"0"` 表示从头。
    #[serde(default)]
    pub introduced: Option<String>,
    /// 修复版本（**开**上界）。
    #[serde(default)]
    pub fixed: Option<String>,
    /// 最后一个受影响版本（**闭**上界）。
    #[serde(default)]
    pub last_affected: Option<String>,
}

/// 区间端点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    /// 无界（`introduced: "0"` 或区间未闭合）。
    Unbounded,
    /// 闭端点（`last_affected`）。
    Inclusive(Version),
    /// 开端点（`fixed`）。
    Exclusive(Version),
}

impl Bound {
    /// 下界：`v >= bound`。
    fn admits_lower(&self, v: &Version) -> bool {
        match self {
            Bound::Unbounded => true,
            Bound::Inclusive(b) => v >= b,
            Bound::Exclusive(b) => v > b,
        }
    }

    /// 上界：`v <= bound` / `v < bound`。
    fn admits_upper(&self, v: &Version) -> bool {
        match self {
            Bound::Unbounded => true,
            Bound::Inclusive(b) => v <= b,
            Bound::Exclusive(b) => v < b,
        }
    }
}

fn parse_version(raw: &str) -> Result<Version, String> {
    // `"0"` 是 OSV 的「从头」哨兵；`"0.0.0-0"` 是合法 semver（小于 0.0.0 的预发布），直接解析。
    if raw == "0" {
        return Version::parse("0.0.0-0").map_err(|e| format!("哨兵 0 解析不了：{e}"));
    }
    Version::parse(raw).map_err(|e| format!("版本 {raw} 不是 semver：{e}"))
}

/// 把事件序列折成 `[下界, 上界]` 区间表。形状不对（如 `fixed` 先于 `introduced`）⇒ `Err`。
pub fn intervals(events: &[OsvEvent]) -> Result<Vec<(Bound, Bound)>, String> {
    let mut out = Vec::new();
    let mut lower: Option<Bound> = None;
    for e in events {
        if let Some(i) = &e.introduced {
            if lower.is_some() {
                return Err(
                    "事件序列里出现两段未闭合的区间（introduced 连着 introduced）".to_string(),
                );
            }
            lower = Some(if i == "0" {
                Bound::Unbounded
            } else {
                Bound::Inclusive(parse_version(i)?)
            });
        } else if let Some(f) = &e.fixed {
            let lo = lower.take().ok_or("fixed 事件没有前置的 introduced")?;
            out.push((lo, Bound::Exclusive(parse_version(f)?)));
        } else if let Some(l) = &e.last_affected {
            let lo = lower
                .take()
                .ok_or("last_affected 事件没有前置的 introduced")?;
            out.push((lo, Bound::Inclusive(parse_version(l)?)));
        } else {
            return Err(format!(
                "无法识别的事件（只认 introduced/fixed/last_affected）：{e:?}"
            ));
        }
    }
    if let Some(lo) = lower {
        out.push((lo, Bound::Unbounded));
    }
    Ok(out)
}

impl Affected {
    /// 这个 affected 条目是否覆盖某版本。`versions` 非空时以它为准（OSV 语义）。
    pub fn covers(&self, v: &Version) -> Result<bool, String> {
        if !self.versions.is_empty() {
            for raw in &self.versions {
                if parse_version(raw)? == *v {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        for range in &self.ranges {
            if range.kind != "SEMVER" {
                continue;
            }
            for (lo, hi) in intervals(&range.events)? {
                if lo.admits_lower(v) && hi.admits_upper(v) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl OsvRecord {
    /// 这条 advisory 是否覆盖某包某版本（按 `ecosystem` + `name` 过滤）。
    pub fn covers(&self, ecosystem: &str, name: &str, v: &Version) -> Result<bool, String> {
        for aff in &self.affected {
            if aff.package.ecosystem != ecosystem || aff.package.name != name {
                continue;
            }
            if aff.covers(v)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 一行给人看的严重度：CVSS 向量优先，退化到 `database_specific.severity`，
    /// RustSec 的 informational（unmaintained 一类）如实带出。
    /// RustSec 的 informational 标记（`unsound` / `unmaintained` / `notice`），没有则 `None`。
    /// 语义是「这条不是漏洞通告」；判不判红由调用方定，本层只如实取出来。
    pub fn informational(&self) -> Option<String> {
        self.affected.iter().find_map(|aff| {
            aff.database_specific
                .as_ref()
                .and_then(|d| d.get("informational"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
    }

    /// 一行给人看的严重度：CVSS 向量优先，退化到 `database_specific.severity`，
    /// 再退化到 RustSec 的 informational 标记。
    pub fn severity_label(&self) -> String {
        if let Some(s) = self.severity.first() {
            return format!("{} {}", s.kind, s.score);
        }
        let spec = self.database_specific.as_ref();
        if let Some(lvl) = spec
            .and_then(|d| d.get("severity"))
            .and_then(|s| s.as_str())
        {
            return lvl.to_string();
        }
        if let Some(info) = self.informational() {
            return format!("informational({info})");
        }
        "unknown".to_string()
    }
}

/// 解析一条 advisory（JSON）。`id` 空 ⇒ 判不了。
pub fn parse_advisory(bytes: &[u8]) -> Result<OsvRecord, String> {
    let rec: OsvRecord =
        serde_json::from_slice(bytes).map_err(|e| format!("advisory 不是合法 OSV JSON：{e}"))?;
    if rec.id.trim().is_empty() {
        return Err("advisory 没有 id".to_string());
    }
    Ok(rec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> Vec<OsvEvent> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn alternating_events_fold_into_multiple_intervals() {
        let events = ev(r#"[{"introduced":"0.0.0-0"},{"fixed":"0.2.0"},
                            {"introduced":"0.2.1-0"},{"fixed":"0.2.1"}]"#);
        let spans = intervals(&events).unwrap();
        assert_eq!(spans.len(), 2);
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(spans[0].0.admits_lower(&v("0.1.0")) && spans[0].1.admits_upper(&v("0.1.0")));
        assert!(
            !spans[0].1.admits_upper(&v("0.2.0")),
            "fixed 是开区间：0.2.0 不该受影响"
        );
        assert!(spans[1].0.admits_lower(&v("0.2.1-0")));
        assert!(
            !spans[1].1.admits_upper(&v("0.2.1")),
            "0.2.1 正好是 fixed 边界"
        );
    }

    #[test]
    fn last_affected_is_a_closed_upper_bound() {
        let events = ev(r#"[{"introduced":"0"},{"last_affected":"0.3.24"}]"#);
        let spans = intervals(&events).unwrap();
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(
            spans[0].1.admits_upper(&v("0.3.24")),
            "last_affected 自己仍受影响"
        );
        assert!(!spans[0].1.admits_upper(&v("0.3.25")));
    }

    #[test]
    fn malformed_event_sequences_are_refused() {
        assert!(
            intervals(&ev(r#"[{"fixed":"1.0.0"}]"#)).is_err(),
            "fixed 没有 introduced 要判不了"
        );
        assert!(
            intervals(&ev(r#"[{"introduced":"0"},{"introduced":"1.0.0"}]"#)).is_err(),
            "两段未闭合区间要判不了"
        );
        assert!(
            intervals(&ev(r#"[{"limit":"1.0.0"}]"#)).is_err(),
            "不认得的键要判不了"
        );
    }

    #[test]
    fn explicit_version_list_wins_over_ranges() {
        let aff: Affected = serde_json::from_str(
            r#"{"package":{"name":"x","ecosystem":"crates.io"},
                "versions":["1.0.0"],
                "ranges":[{"type":"SEMVER","events":[{"introduced":"0"}]}]}"#,
        )
        .unwrap();
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(aff.covers(&v("1.0.0")).unwrap());
        assert!(!aff.covers(&v("2.0.0")).unwrap(), "显式表非空时不看 ranges");
    }
}
#[test]
fn exclusive_lower_bound_is_pinned_even_though_no_range_produces_it() {
    // 现行程表只用 `Inclusive`（introduced）/`Unbounded`（哨兵）做下界，`Exclusive` 那条臂
    // 是"给未来留的"——变异门实测它上面的三个比较变异全都杀不掉（没有输入走到它）。
    // 判据直接钉在类型上：下界 `Exclusive(b)` 的含义是 `v > b`（开）。
    let v = |s: &str| Version::parse(s).unwrap();
    let b = Bound::Exclusive(v("1.0.0"));
    assert!(!b.admits_lower(&v("0.9.9")), "小于 ⇒ 不在内");
    assert!(!b.admits_lower(&v("1.0.0")), "等于 ⇒ 不在内（开区间）");
    assert!(b.admits_lower(&v("1.0.1")), "大于 ⇒ 在内");
    let incl = Bound::Inclusive(v("1.0.0"));
    assert!(incl.admits_lower(&v("1.0.0")), "闭区间含端点");
    assert!(Bound::Unbounded.admits_lower(&v("0.0.1")), "无界下界恒真");
}

#[test]
fn covers_filters_by_ecosystem_as_well_as_name() {
    // 变异门抓到过 `||`→`&&`：那样"名字对但生态不对"的条目会被误当成命中。这条钉住两侧过滤。
    let rec: OsvRecord = serde_json::from_str(
        r#"{"id":"RUSTSEC-2099-0009","affected":[
                 {"package":{"name":"time","ecosystem":"crates.io"},
                  "ranges":[{"type":"SEMVER","events":[{"introduced":"0"}]}]}]}"#,
    )
    .unwrap();
    let v = Version::parse("0.1.44").unwrap();
    assert!(rec.covers("crates.io", "time", &v).unwrap());
    assert!(
        !rec.covers("PyPI", "time", &v).unwrap(),
        "生态不对 ⇒ 不许命中"
    );
    assert!(
        !rec.covers("crates.io", "other", &v).unwrap(),
        "名字不对 ⇒ 不许命中"
    );
}
