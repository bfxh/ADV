//! OSV 快照的同步与记账——`adv snapshot` 的**网络面**（离线判定永不碰这里）。
//!
//! 为什么不是"下载 all.zip 再解包"（2026-10-08 实测改的口径）：
//! - crates.io 生态在 GCS 上是**逐对象**的：`?list-type=2&prefix=crates.io/` 三页列完 **2898 个对象**，
//!   每个对象自带 `ETag` / `Size` / `LastModified`（单文件实测 200 + ETag，条件 GET 实测 304）。
//! - 所以快照 = **列目录 + 逐对象条件 GET**：首装 2898 个请求、之后只拉 ETag 变了的；
//!   完全不需要 zip 读取器（也就没有"手写 ZIP 还是引 zip crate"这个岔路）。
//! - **发布方没有签名**（`all.zip.sha256` / `.sig` / `manifest.json` 实测全 404）。能做到的最强核对是：
//!   GCS 的 ETag 对普通对象就是**内容的 MD5** ⇒ 落地时重算 MD5 与 ETag 比对，不一致即判不了并保留旧件。
//!   本层如实登记"无签名"，只承诺「传输完整性 + 变更留痕」，不假装验签。
//!
//! 依赖外部的 `curl`（本仓无 HTTP 客户端；与深轨 shell out 到 cargo/rustc 同一形态）。
//! `curl` 不在 PATH ⇒ **判不了**（`Err`），不静默跳过。

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

/// OSV 公开桶的根（GCS；S3 兼容 V2 列表接口）。
pub const DEFAULT_BASE: &str = "https://osv-vulnerabilities.storage.googleapis.com/";
/// 只同步的生态前缀（本层只做 crates.io）。
pub const DEFAULT_PREFIX: &str = "crates.io/";
const MANIFEST: &str = "manifest.json";

/// 列表里的一行（GCS V2 XML 的 `<Contents>` 四件套）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedObject {
    /// 对象键（`crates.io/<ID>.json`）。
    pub key: String,
    /// 发布方给的 ETag（普通对象就是内容的 MD5，十六进制、无引号）。
    pub etag: String,
    /// 字节数（与落地文件对账）。
    pub size: u64,
    /// 上游最后修改时间（原样带出）。
    pub last_modified: String,
}

/// 快照里一个对象的记账（`md5` 是我们落地时重算的，与 `etag` 比对后才写）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectMeta {
    /// 发布方 ETag（与落地 md5 比对过才写）。
    pub etag: String,
    /// 落地时重算的 MD5（十六进制）。
    pub md5: String,
    /// 落地字节数。
    pub size: u64,
    /// 记下这份内容对应的上游时间。
    pub last_modified: String,
}

/// 第二路的同步记账（RustSec advisory-db）：钉的是 **commit SHA**——那一侧没有每文件 ETag，
/// 完整性凭据就是 git 历史本身。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustSecMeta {
    /// 同步时的 HEAD commit。
    pub sha: String,
    /// 读到的公告条数。
    pub advisories: usize,
    /// 首次克隆还是原地更新。
    pub fresh_clone: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
/// 快照目录的记账（`manifest.json`）：这是我们唯一的"变更留痕"载体。
pub struct Manifest {
    #[serde(default)]
    /// 桶地址（换源时账要对得上）。
    pub source: String,
    #[serde(default)]
    /// 生态前缀。
    pub prefix: String,
    #[serde(default)]
    /// 键 → 对象记账（BTreeMap ⇒ 落盘顺序稳定，diff 可读）。
    pub objects: BTreeMap<String, ObjectMeta>,
    #[serde(default)]
    /// RustSec 路的记账（没同步过就是 `None`）。
    pub rustsec: Option<RustSecMeta>,
}

#[derive(Debug, Default)]
/// 一次同步的读数（给人看的一行账的原料）。
pub struct SyncReport {
    /// 上游这次列出的对象数。
    pub listed: usize,
    /// 真下载并替换的个数。
    pub fetched: usize,
    /// ETag 未变（或 304）而跳过的个数。
    pub unchanged: usize,
    /// 记账里有、上游这次没列出来的键（只报不删，删不删由人决定）。
    pub removed_upstream: Vec<String>,
}

/// 解析一页列表 XML。**形状不对即判不了**（拿正则当解析器，只认 GCS 这份机生成的扁平形状）。
pub fn parse_listing_page(xml: &str) -> Result<(Vec<ListedObject>, Option<String>), String> {
    if !xml.contains("<ListBucketResult") {
        return Err(format!(
            "不是 GCS 列表响应（开头 80 字符）：{}",
            xml.chars().take(80).collect::<String>()
        ));
    }
    let mut out = Vec::new();
    for block in xml.split("<Contents>").skip(1) {
        let block = block.split("</Contents>").next().unwrap_or("");
        let field = |tag: &str| -> Option<String> {
            let start = block.find(&format!("<{tag}>"))? + tag.len() + 2;
            let end = block[start..].find(&format!("</{tag}>"))? + start;
            Some(block[start..end].to_string())
        };
        let key = field("Key").ok_or("列表块缺 <Key>")?;
        let etag = field("ETag")
            .ok_or("列表块缺 <ETag>")?
            .trim_matches('"')
            .to_string();
        let size = field("Size").ok_or("列表块缺 <Size>")?;
        let last_modified = field("LastModified").ok_or("列表块缺 <LastModified>")?;
        out.push(ListedObject {
            key,
            etag,
            size: size
                .parse()
                .map_err(|e| format!("<Size> 不是数字：{size}（{e}）"))?,
            last_modified,
        });
    }
    let next = xml
        .split("<NextContinuationToken>")
        .nth(1)
        .and_then(|s| s.split("</NextContinuationToken>").next())
        .map(str::to_string);
    Ok((out, next))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 内容的 MD5（十六进制小写）——与 GCS 的 ETag 同算法，落地后拿它对账。
pub fn md5_hex(bytes: &[u8]) -> String {
    hex(&Md5::digest(bytes))
}

/// 抓取面的**缝**：生产的 [`CurlFetcher`] 起外部 curl；测试注入假实现。
///
/// 为什么要有这条缝（变异门实测）：`sync` 原先把 host 写死、抓取直连 curl，于是
/// 「listing / 条件 GET / MD5 核对 / 并发 / 落地 / 记账」整条路径**一条断言都没有**——
/// 收尾那一轮里这一片冒出 ~35 条存活变异，全落在这里。缝开出来之后，测试用假 fetcher
/// 就能把这些分支逐条钉住；`CurlFetcher` 那一层薄壳另用真 curl + `file://` 测。
pub trait Fetcher: Sync {
    /// 抓一个 URL。`etag` 非空则带 `If-None-Match`；`out` 非空则把响应体落盘。
    /// 返回 (状态码, 响应体)。
    fn get(
        &self,
        url: &str,
        etag: Option<&str>,
        out: Option<&Path>,
    ) -> Result<(u16, Vec<u8>), String>;
}

/// 生产实现：外部 curl。
pub struct CurlFetcher;

impl Fetcher for CurlFetcher {
    fn get(
        &self,
        url: &str,
        etag: Option<&str>,
        out: Option<&Path>,
    ) -> Result<(u16, Vec<u8>), String> {
        let mut extra: Vec<String> = Vec::new();
        if let Some(path) = out {
            extra.push("-o".into());
            extra.push(path.to_string_lossy().into_owned());
            extra.push("--create-dirs".into());
        }
        if let Some(tag) = etag {
            extra.push("-H".into());
            extra.push(format!("If-None-Match: \"{tag}\""));
        }
        let refs: Vec<&str> = extra.iter().map(String::as_str).collect();
        curl(url, &refs)
    }
}

fn curl(url: &str, extra: &[&str]) -> Result<(u16, Vec<u8>), String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-L", "--max-time", "120", "-w", "\n%{http_code}"]);
    cmd.args(extra);
    cmd.arg(url);
    let out = cmd
        .output()
        .map_err(|e| format!("起不了 curl（{e}）——本层依赖外部 curl，PATH 里没有就是判不了"))?;
    let stdout = out.stdout;
    // `-w` 把状态码写在最后一行；正文可能含换行，按最后一个换行切。
    let pos = stdout
        .iter()
        .rposition(|b| *b == b'\n')
        .ok_or("curl 输出里没有状态码行")?;
    // 这里刻意不做 `pos + 1`：状态码前那个换行由下面的 `trim()` 吃掉，写成 `pos + 1` 只是
    // 多一个**等价变异**（变异门实测它永远杀不掉——两种写法对同一输入同值）。
    let code: u16 = String::from_utf8_lossy(&stdout[pos..])
        .trim()
        .parse()
        .map_err(|e| format!("curl 状态码读不出：{e}"))?;
    // Windows 的 curl 在 `-w` 输出前打的是 **CRLF**，按 `\n` 切之后正文会留一个尾随 `\r`
    // （`file://` 的适配器测试抓到的：带 `-o` 时正文本该为空，实测是 `"\r"`）。切掉它。
    let mut body = stdout[..pos].to_vec();
    if body.last() == Some(&b'\r') {
        body.pop();
    }
    Ok((code, body))
}

fn list_all(base: &str, prefix: &str, fetcher: &dyn Fetcher) -> Result<Vec<ListedObject>, String> {
    let mut all = Vec::new();
    let mut token: Option<String> = None;
    for _page in 0..200 {
        let mut url = format!("{base}?list-type=2&prefix={prefix}");
        if let Some(t) = &token {
            url.push_str(&format!("&continuation-token={}", urlencode(t)));
        }
        let (code, body) = fetcher.get(&url, None, None)?;
        if code != 200 {
            return Err(format!("列目录返回 {code}"));
        }
        let xml = String::from_utf8_lossy(&body).into_owned();
        let (page, next) = parse_listing_page(&xml)?;
        all.extend(page);
        match next {
            Some(t) => token = Some(t),
            None => return Ok(all),
        }
    }
    Err("列表翻页超过 200 页还没完（形状可疑）".to_string())
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            // 空格不需要单独一条臂：`%{b:02X}` 对 0x20 正好打出 `%20`（同值分支=等价变异）。
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// 读记账；文件不存在 ⇒ 空记账（首装）。
pub fn load_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST);
    if !path.is_file() {
        return Ok(Manifest::default());
    }
    let raw = std::fs::read(&path).with_context(|| format!("读 {}", path.display()))?;
    serde_json::from_slice(&raw).with_context(|| format!("解析 {}", path.display()))
}

/// 把 RustSec 路的同步结果写进同一份记账（`manifest.json` 的 `rustsec` 段）。
pub fn record_rustsec(dir: &Path, meta: RustSecMeta) -> Result<()> {
    let mut m = load_manifest(dir)?;
    m.rustsec = Some(meta);
    save_manifest(dir, &m)
}

/// 写记账（整份覆盖；内容是确定性序列化的）。
pub fn save_manifest(dir: &Path, m: &Manifest) -> Result<()> {
    let path = dir.join(MANIFEST);
    let body = serde_json::to_vec_pretty(m)?;
    std::fs::write(&path, body).with_context(|| format!("写 {}", path.display()))
}

/// 并发抓取的工人数。实测单个对象 ~0.8s（curl 起进程 + TLS 握手都算在内）⇒ 串行首装
/// 2898 个要 ~40 分钟，8 路压到 ~5 分钟；再高收益递减（对端与本地进程创建都要钱）。
const WORKERS: usize = 8;

/// 同步一次快照：列目录 → 只拉 ETag 变了的 → 逐对象核 MD5 → 落记账。
///
/// 失败语义：任何一条抓取/核对失败 ⇒ `Err` 且**不写记账**（下次会重来），
/// 不把"抓了一半"记成"同步过了"。
pub fn sync(dir: &Path) -> Result<SyncReport> {
    sync_with(dir, DEFAULT_BASE, DEFAULT_PREFIX, &CurlFetcher)
}

/// 同 [`sync`]，但 host / prefix / 抓取实现都可注入（测试用；生产走 `sync`）。
pub fn sync_with(
    dir: &Path,
    base: &str,
    prefix: &str,
    fetcher: &dyn Fetcher,
) -> Result<SyncReport> {
    let listed = list_all(base, prefix, fetcher).map_err(|e| anyhow!("列目录失败：{e}"))?;
    let mut manifest = load_manifest(dir)?;
    manifest.source = base.to_string();
    manifest.prefix = prefix.to_string();

    let mut report = SyncReport {
        listed: listed.len(),
        ..Default::default()
    };
    // 上游这次列出的**全量**键（不是 todo——todo 只是待抓子集，拿它算"上游撤下"会把未变的都误判成撤下）。
    let listed_all: std::collections::BTreeSet<String> =
        listed.iter().map(|o| o.key.clone()).collect();
    let mut todo: Vec<ListedObject> = Vec::new();
    for obj in listed {
        let target = dir.join(&obj.key);
        let unchanged = manifest
            .objects
            .get(&obj.key)
            .map(|m| m.etag == obj.etag && target.is_file())
            .unwrap_or(false);
        if unchanged {
            report.unchanged += 1;
        } else {
            todo.push(obj);
        }
    }
    let fetched = fetch_all(dir, &todo, &manifest, base, fetcher)?;
    for (key, meta) in fetched {
        manifest.objects.insert(key, meta);
        report.fetched += 1;
    }
    report.removed_upstream = manifest
        .objects
        .keys()
        .filter(|k| !listed_all.contains(*k))
        .cloned()
        .collect();
    save_manifest(dir, &manifest)?;
    Ok(report)
}

/// 8 路并发抓 `todo`；任何一条失败 ⇒ 整体 `Err`（附上失败键）。
fn fetch_all(
    dir: &Path,
    todo: &[ListedObject],
    manifest: &Manifest,
    base: &str,
    fetcher: &dyn Fetcher,
) -> Result<Vec<(String, ObjectMeta)>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done: std::sync::Mutex<Vec<(String, ObjectMeta)>> = std::sync::Mutex::new(Vec::new());
    let fails: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(todo.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(obj) = todo.get(i) else { break };
                    match fetch_one(
                        dir,
                        obj,
                        manifest.objects.get(&obj.key).map(|m| m.etag.as_str()),
                        base,
                        fetcher,
                    ) {
                        Ok(Some(meta)) => done.lock().unwrap().push((obj.key.clone(), meta)),
                        Ok(None) => {} // 304：上游说没变
                        Err(e) => fails.lock().unwrap().push(format!("{}：{e}", obj.key)),
                    }
                }
            });
        }
    });
    let fails = fails.into_inner().unwrap();
    if !fails.is_empty() {
        return Err(anyhow!(
            "{} 条抓取失败（记账未写，下次重来）：{}",
            fails.len(),
            fails.join("；")
        ));
    }
    let mut done = done.into_inner().unwrap();
    done.sort_by(|a, b| a.0.cmp(&b.0)); // 只按键排序，落盘顺序才确定
    Ok(done)
}

/// 抓一个对象：条件 GET → 304 返 `None`；200 则核 MD5/字节数 → 原子替换 → 返记账。
fn fetch_one(
    dir: &Path,
    obj: &ListedObject,
    known_etag: Option<&str>,
    base: &str,
    fetcher: &dyn Fetcher,
) -> Result<Option<ObjectMeta>> {
    let url = format!("{base}{}", obj.key);
    let target = dir.join(&obj.key);
    let tmp = target.with_extension("part");
    let (code, _) = fetcher
        .get(&url, known_etag, Some(&tmp))
        .map_err(|e| anyhow!("{e}"))?;
    if code == 304 {
        let _ = std::fs::remove_file(&tmp);
        return Ok(None);
    }
    if code != 200 {
        return Err(anyhow!("返回 {code}（不把失败读成没有）"));
    }
    let bytes = std::fs::read(&tmp).with_context(|| format!("读回 {}", tmp.display()))?;
    let got = md5_hex(&bytes);
    if got != obj.etag.to_lowercase() {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!(
            "内容 MD5 {got} 与发布方 ETag {} 不符（旧件保留不替换）",
            obj.etag
        ));
    }
    if bytes.len() as u64 != obj.size {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("字节数 {} 与列表 {} 不符", bytes.len(), obj.size));
    }
    std::fs::rename(&tmp, &target).with_context(|| format!("落地 {}", target.display()))?;
    Ok(Some(ObjectMeta {
        etag: obj.etag.clone(),
        md5: got,
        size: obj.size,
        last_modified: obj.last_modified.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_page_extracts_the_four_fields_and_the_token() {
        let xml = r#"<?xml version='1.0'?><ListBucketResult><Name>b</Name>
            <NextContinuationToken>TOK==</NextContinuationToken><KeyCount>1</KeyCount>
            <Contents><Key>crates.io/A.json</Key><Generation>1</Generation><MetaGeneration>1</MetaGeneration>
            <LastModified>2026-09-10T04:00:13.326Z</LastModified><ETag>"abc123"</ETag><Size>1442</Size></Contents>
            </ListBucketResult>"#;
        let (objs, token) = parse_listing_page(xml).unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].key, "crates.io/A.json");
        assert_eq!(objs[0].etag, "abc123", "ETag 的引号要剥掉");
        assert_eq!(objs[0].size, 1442);
        assert_eq!(token.as_deref(), Some("TOK=="));
    }

    #[test]
    fn a_non_listing_body_is_refused_not_read_as_empty() {
        let err = parse_listing_page("<html>404 Not Found</html>").unwrap_err();
        assert!(err.contains("不是 GCS 列表响应"), "{err}");
    }

    #[test]
    fn a_contents_block_missing_a_field_is_refused() {
        let xml = "<ListBucketResult><Contents><Key>k</Key><ETag>\"e\"</ETag></Contents></ListBucketResult>";
        assert!(parse_listing_page(xml).unwrap_err().contains("<Size>"));
    }

    #[test]
    fn md5_matches_the_known_digest_of_an_empty_input() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    }
}
