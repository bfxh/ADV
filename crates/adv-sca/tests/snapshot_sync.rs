//! 抓取面（`sync` 全路径）的金丝雀——缝是 `snapshot::Fetcher`。
//!
//! 为什么单独一个文件：变异门实测这一片**一条断言都没有**（listing / 条件 GET / MD5 核对 / 并发 /
//! 落地 / 记账 / 上游撤下，~35 条存活变异全落在这里），因为原先把 host 写死、抓取直连 curl。
//! 现在测试注入假 fetcher 把这些分支逐条钉住；`CurlFetcher` 那层薄壳另用**真 curl + `file://`** 测。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use adv_sca::snapshot::{Fetcher, RustSecMeta, load_manifest, md5_hex, record_rustsec, sync_with};

const BASE: &str = "https://bucket.invalid/";
const PREFIX: &str = "crates.io/";

/// 假桶：按 URL 给列表页与对象；`etag` 命中就回 304（不落盘）。
struct FakeBucket {
    /// 首页与后续页的键（第一页带 token）。
    pages: Vec<Vec<String>>,
    objects: BTreeMap<String, Vec<u8>>,
    /// 这些键的抓取直接失败（测失败语义）。
    fail_keys: Vec<String>,
}

impl FakeBucket {
    fn new(objects: &[(&str, &[u8])]) -> Self {
        let keys: Vec<String> = objects.iter().map(|(k, _)| (*k).to_string()).collect();
        Self {
            pages: vec![keys],
            objects: objects
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.to_vec()))
                .collect(),
            fail_keys: Vec::new(),
        }
    }

    fn with_pages(pages: &[&[&str]], objects: &[(&str, &[u8])]) -> Self {
        let mut b = Self::new(objects);
        b.pages = pages
            .iter()
            .map(|p| p.iter().map(|k| (*k).to_string()).collect())
            .collect();
        b
    }

    fn listing_xml(&self, page: usize) -> String {
        let mut xml = String::from(
            "<?xml version='1.0'?><ListBucketResult><Name>b</Name><KeyCount>0</KeyCount>",
        );
        if let Some(next) = self.pages.get(page + 1) {
            xml.push_str(&format!(
                "<NextContinuationToken>tok{next:?}</NextContinuationToken>"
            ));
        }
        for key in self.pages.get(page).cloned().unwrap_or_default() {
            let body = self.objects.get(&key).cloned().unwrap_or_default();
            xml.push_str(&format!(
                "<Contents><Key>{key}</Key><LastModified>2026-10-08T00:00:00Z</LastModified>\
                 <ETag>\"{}\"</ETag><Size>{}</Size></Contents>",
                md5_hex(&body),
                body.len()
            ));
        }
        xml.push_str("</ListBucketResult>");
        xml
    }
}

impl Fetcher for FakeBucket {
    fn get(
        &self,
        url: &str,
        etag: Option<&str>,
        out: Option<&Path>,
    ) -> Result<(u16, Vec<u8>), String> {
        if url.contains("?list-type=2") {
            // 第 0 页；带 continuation-token 的请求回第 1 页（测试只用到两页）。
            let page = if url.contains("continuation-token=") {
                1
            } else {
                0
            };
            return Ok((200, self.listing_xml(page).into_bytes()));
        }
        let key = url
            .strip_prefix(BASE)
            .ok_or_else(|| format!("假桶不认这个 URL：{url}"))?;
        if self.fail_keys.iter().any(|k| k == key) {
            return Err(format!("假桶：{key} 抓取失败"));
        }
        let body = self
            .objects
            .get(key)
            .ok_or_else(|| format!("假桶里没有 {key}"))?
            .clone();
        if etag == Some(md5_hex(&body).as_str()) {
            return Ok((304, Vec::new()));
        }
        if let Some(path) = out {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, &body).map_err(|e| e.to_string())?;
        }
        Ok((200, body))
    }
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adv-snapshot-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn sync_writes_files_and_manifest_and_counts_what_it_fetched() {
    let dir = tmp("fetch");
    let bucket = FakeBucket::new(&[("crates.io/A.json", b"aa"), ("crates.io/B.json", b"bbbb")]);
    let report = sync_with(&dir, BASE, PREFIX, &bucket).unwrap();
    assert_eq!((report.listed, report.fetched, report.unchanged), (2, 2, 0));
    assert_eq!(std::fs::read(dir.join("crates.io/A.json")).unwrap(), b"aa");
    let m = load_manifest(&dir).unwrap();
    assert_eq!(m.source, BASE);
    assert_eq!(m.prefix, PREFIX);
    let meta = m.objects.get("crates.io/B.json").unwrap();
    assert_eq!(meta.size, 4);
    assert_eq!(meta.md5, md5_hex(b"bbbb"));
    assert_eq!(
        meta.etag, meta.md5,
        "记账里 etag 与 md5 必须一致（GCS 语义）"
    );
}

#[test]
fn a_second_sync_is_a_no_op_when_the_etags_still_match() {
    let dir = tmp("noop");
    let bucket = FakeBucket::new(&[("crates.io/A.json", b"aa")]);
    assert_eq!(sync_with(&dir, BASE, PREFIX, &bucket).unwrap().fetched, 1);
    let again = sync_with(&dir, BASE, PREFIX, &bucket).unwrap();
    assert_eq!(
        (again.fetched, again.unchanged),
        (0, 1),
        "304 走 unchanged 分支"
    );
}

#[test]
fn pagination_follows_the_continuation_token() {
    let dir = tmp("pages");
    let bucket = FakeBucket::with_pages(
        &[&["crates.io/A.json"], &["crates.io/B.json"]],
        &[("crates.io/A.json", b"a"), ("crates.io/B.json", b"b")],
    );
    let report = sync_with(&dir, BASE, PREFIX, &bucket).unwrap();
    assert_eq!((report.listed, report.fetched), (2, 2));
}

#[test]
fn a_md5_mismatch_is_refused_and_the_old_file_stays() {
    let dir = tmp("md5");
    std::fs::create_dir_all(dir.join("crates.io")).unwrap();
    std::fs::write(dir.join("crates.io/A.json"), "旧件".as_bytes()).unwrap();
    struct Liar;
    impl Fetcher for Liar {
        fn get(
            &self,
            url: &str,
            _e: Option<&str>,
            out: Option<&Path>,
        ) -> Result<(u16, Vec<u8>), String> {
            if url.contains("?list-type=2") {
                return Ok((
                    200,
                    b"<ListBucketResult><Contents><Key>crates.io/A.json</Key>\
                    <LastModified>t</LastModified><ETag>\"deadbeef\"</ETag><Size>3</Size>\
                    </Contents></ListBucketResult>"
                        .to_vec(),
                ));
            }
            if let Some(p) = out {
                std::fs::write(p, b"abc").unwrap();
            }
            Ok((200, b"abc".to_vec()))
        }
    }
    let err = sync_with(&dir, BASE, PREFIX, &Liar).unwrap_err();
    assert!(err.to_string().contains("MD5"), "{err}");
    assert_eq!(
        std::fs::read(dir.join("crates.io/A.json")).unwrap(),
        "旧件".as_bytes(),
        "旧件必须保留"
    );
}

#[test]
fn a_size_mismatch_is_refused() {
    let dir = tmp("size");
    struct Short;
    impl Fetcher for Short {
        fn get(
            &self,
            url: &str,
            _e: Option<&str>,
            out: Option<&Path>,
        ) -> Result<(u16, Vec<u8>), String> {
            if url.contains("?list-type=2") {
                // ETag 与内容一致，但 Size 报大了一码
                return Ok((
                    200,
                    format!(
                        "<ListBucketResult><Contents><Key>crates.io/A.json</Key>\
                    <LastModified>t</LastModified><ETag>\"{}\"</ETag><Size>9</Size></Contents>\
                    </ListBucketResult>",
                        md5_hex(b"ab")
                    )
                    .into_bytes(),
                ));
            }
            if let Some(p) = out {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::write(p, b"ab").unwrap();
            }
            Ok((200, b"ab".to_vec()))
        }
    }
    let err = sync_with(&dir, BASE, PREFIX, &Short).unwrap_err();
    assert!(err.to_string().contains("字节数"), "{err}");
}

#[test]
fn a_failing_object_fails_the_whole_sync_and_writes_no_manifest() {
    let dir = tmp("fail");
    let mut bucket = FakeBucket::new(&[("crates.io/A.json", b"a"), ("crates.io/B.json", b"b")]);
    bucket.fail_keys.push("crates.io/B.json".into());
    let err = sync_with(&dir, BASE, PREFIX, &bucket).unwrap_err();
    assert!(err.to_string().contains("抓取失败"), "{err}");
    assert!(
        !dir.join("manifest.json").is_file(),
        "抓了一半不算同步过：记账不该落盘"
    );
}

#[test]
fn keys_gone_from_the_listing_are_reported_as_removed_upstream() {
    let dir = tmp("removed");
    let two = FakeBucket::new(&[("crates.io/A.json", b"a"), ("crates.io/B.json", b"b")]);
    assert_eq!(sync_with(&dir, BASE, PREFIX, &two).unwrap().fetched, 2);
    let one = FakeBucket::new(&[("crates.io/A.json", b"a")]);
    let report = sync_with(&dir, BASE, PREFIX, &one).unwrap();
    assert_eq!(
        report.removed_upstream,
        vec!["crates.io/B.json".to_string()]
    );
    assert_eq!(report.unchanged, 1);
}

#[test]
fn urlencode_escapes_reserved_bytes_but_keeps_the_safe_set() {
    // 只通过行为测：token 里有空格与 '=' 时，请求 URL 必须被转义（假桶把它们读回来）。
    let dir = tmp("urlencode");
    struct Echo;
    impl Fetcher for Echo {
        fn get(
            &self,
            url: &str,
            _e: Option<&str>,
            _o: Option<&Path>,
        ) -> Result<(u16, Vec<u8>), String> {
            if url.contains("continuation-token=") {
                assert!(url.ends_with("continuation-token=tok%20a%3Db"), "{url}");
                return Ok((200, b"<ListBucketResult></ListBucketResult>".to_vec()));
            }
            Ok((
                200,
                b"<ListBucketResult><NextContinuationToken>tok a=b</NextContinuationToken>\
                </ListBucketResult>"
                    .to_vec(),
            ))
        }
    }
    let report = sync_with(&dir, BASE, PREFIX, &Echo).unwrap();
    assert_eq!(report.listed, 0);
}

#[test]
fn the_curl_adapter_reads_a_real_file_url_and_parses_the_status_line() {
    // 薄壳测试：用真 curl 抓 `file://`（无查询串的那条路径），核对正文与状态码行。
    use adv_sca::snapshot::CurlFetcher;
    let dir = tmp("curl");
    let src = dir.join("src.json");
    std::fs::write(&src, b"hello-osv").unwrap();
    let url = format!("file:///{}", src.to_string_lossy().replace('\\', "/"));
    let out = dir.join("copy.json");
    let (code, body) = CurlFetcher.get(&url, None, Some(&out)).unwrap();
    // `file://` 没有 HTTP 状态码（`%{http_code}` 打 000 ⇒ 解析出 0）。这条测的是**适配器的解析形状**
    // （按最后一个换行切正文/状态码、`-o` 落盘）；200/304 的语义由上面那些假 fetcher 的用例覆盖。
    assert_eq!(code, 0, "file:// 的 http_code 是 000（解析成 0），不是 200");
    assert!(body.is_empty(), "带 -o 时正文落盘、stdout 只剩状态码行");
    assert_eq!(std::fs::read(&out).unwrap(), b"hello-osv");
    let missing = format!(
        "file:///{}",
        dir.join("nope.json").to_string_lossy().replace('\\', "/")
    );
    // 抓不到的那条只断言"不是成功态"：`file://` 下成功/失败都打 000，这条区分不了具体原因，
    // 真正的 200/304/其它码语义由假 fetcher 的用例覆盖（不在这里假装测到了）。
    let (code, _) = CurlFetcher.get(&missing, None, None).unwrap();
    assert_ne!(code, 200, "抓不到时不许回 200");
}

#[test]
fn the_manifest_round_trips_and_carries_the_rustsec_pin() {
    let dir = tmp("manifest");
    let meta = RustSecMeta {
        sha: "deadbeef".into(),
        advisories: 1274,
        fresh_clone: true,
    };
    record_rustsec(&dir, meta.clone()).unwrap();
    let m = load_manifest(&dir).unwrap();
    assert_eq!(m.rustsec, Some(meta));
}
