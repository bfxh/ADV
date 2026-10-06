//! 金丝雀：证明门会红（旧仓纪律「金丝雀先记基线再验红」——全绿的门不值得信）。

use std::collections::BTreeMap;
use xtask::god::{MAX_FN_LINES, analyze_source, hard_violations, ratchet_violations};
use xtask::lockstep::check_lockstep;
use xtask::mutants::{
    df_args, disk_failure_signature, drive_letter, free_bytes_probe, human_bytes, keys_by_summary,
    min_free_gib, new_missed, parse_df_avail, parse_u64_lines, powershell_args, scratch_is_short,
    short_message, tally, unverifiable_keys, unviable_disk_failures,
};
use xtask::suppress::file_violations;

fn long_fn_source(lines: usize) -> String {
    let mut s = String::from("fn big() {\n");
    for i in 0..lines {
        s.push_str(&format!("    let _x{i} = {i}; // filler\n"));
    }
    s.push_str("}\n");
    s
}

#[test]
fn canary_long_fn_is_caught() {
    let over = MAX_FN_LINES as usize + 10;
    let src = format!("mod m {{\n{}\n}}\n", long_fn_source(over));
    let entries = analyze_source("crates/fake/src/canary.rs", &src);
    let caught = entries
        .iter()
        .filter(|(k, _)| k.starts_with("fn:"))
        .any(|(_, v)| *v > MAX_FN_LINES);
    assert!(
        caught,
        "金丝雀失败：{} 行的函数没被抓到（阈 {MAX_FN_LINES}）",
        over
    );
    assert!(
        !hard_violations(&entries).is_empty(),
        "金丝雀失败：硬阈检查没有报红"
    );
}

#[test]
fn canary_impl_members_are_counted() {
    let src = r#"
struct S { a: u32, b: u32, c: u32 }
impl S {
    fn one(&self) -> u32 { self.a }
    fn two(&self) -> u32 { self.b }
    fn three(&self) -> u32 { self.c }
}
"#;
    let entries = analyze_source("crates/fake/src/impl_canary.rs", src);
    assert_eq!(entries["type:crates/fake/src/impl_canary.rs::S(impl)"], 3);
    assert_eq!(entries["type:crates/fake/src/impl_canary.rs::S(fields)"], 3);
    assert!(entries.contains_key("fn:crates/fake/src/impl_canary.rs::S::two"));
}

#[test]
fn canary_ratchet_fails_on_growth_and_unregistered() {
    let mut baseline = BTreeMap::new();
    baseline.insert("fn:a.rs::f".to_string(), 10);
    let mut current = BTreeMap::new();
    current.insert("fn:a.rs::f".to_string(), 11); // 恶化
    current.insert("fn:a.rs::g".to_string(), 3); // 新面未登记
    let v = ratchet_violations(&current, &baseline);
    assert_eq!(v.len(), 2, "棘轮必须同时抓恶化与未登记：{v:?}");

    // 只准减：基线大于当前 = 绿
    let mut smaller = BTreeMap::new();
    smaller.insert("fn:a.rs::f".to_string(), 9);
    assert!(ratchet_violations(&smaller, &baseline).is_empty());
}

#[test]
fn canary_lockstep_rejects_drift() {
    // 引导期：无 adv-v tag 只许 0.1.0
    assert!(check_lockstep("0.1.0", &[]).is_ok());
    assert!(check_lockstep("0.2.0", &[]).is_err());
    // 旧仓 v* tag 不参与判定（adv-v 前缀隔离，2026-10-04 首跑裁定）
    assert!(check_lockstep("0.1.0", &["v2.58.0".to_string()]).is_ok());
    // 有 adv-v tag：必须精确对齐最新 tag（旧仓 36 minor 断档教训的负样本）
    let tags = vec!["adv-v0.1.0".to_string(), "adv-v0.2.0".to_string()];
    assert!(check_lockstep("0.2.0", &tags).is_ok());
    assert!(check_lockstep("0.3.0", &tags).is_err());
    // 非法版本
    assert!(check_lockstep("abc", &[]).is_err());
}

#[test]
fn canary_expired_suppression_is_red() {
    let src = "fn f() { v.unwrap() } // adv:allow(RS-UNWRAP-USE, reason=旧账, until=2026-01-01)
";
    let v = file_violations("t.rs", src, "2026-10-04");
    assert!(
        v.iter().any(|x| x.contains("抑制到期")),
        "金丝雀失败：过期抑制没被抓到：{v:?}"
    );
}

#[test]
fn canary_valid_suppression_and_malformed() {
    // 有效抑制 = 绿
    let ok = "fn f() { v.unwrap() } // adv:allow(RS-UNWRAP-USE, reason=测试, until=2999-01-01)
";
    assert!(file_violations("t.rs", ok, "2026-10-04").is_empty());
    // 畸形（缺 until）= 红
    let bad = "// adv:allow(RS-UNWRAP-USE, reason=忘了日期)
fn f() {}
";
    let v = file_violations("t.rs", bad, "2026-10-04");
    assert!(
        v.iter().any(|x| x.contains("畸形")),
        "金丝雀失败：畸形抑制没被抓到：{v:?}"
    );
}

#[test]
fn canary_new_missed_mutants_are_red() {
    let baseline = vec!["a.rs::old_fn".to_string()];
    let current = vec![
        "a.rs::old_fn".to_string(),     // 存量债 = 绿
        "taint.rs::new_fn".to_string(), // 新债 = 红
    ];
    let v = new_missed(&current, &baseline);
    assert_eq!(
        v,
        vec!["taint.rs::new_fn".to_string()],
        "新 missed 必须红，存量 missed 绿"
    );
    // 全捕获 = 绿
    assert!(new_missed(&[], &baseline).is_empty());
}

#[test]
fn canary_mutants_flags_baseline_key_that_lost_verifiability() {
    // 真值语料锚到 2026-10-05 实测那一轮：基线含 5 个 xtask 键，而本轮它们的变异
    // 全部没跑到判定环节（判 Unviable——实为 `ld.exe: No space left on device`），
    // 旧判据只看"新增 missed"，那一轮照样报绿。
    let baseline: Vec<String> = ["xtask/src/mir.rs::run", "xtask/src/mutants.rs::missed_key"]
        .into_iter()
        .map(String::from)
        .collect();
    let verifiable: Vec<String> = ["crates/adv-rules/src/taint.rs::expr_taint"]
        .into_iter()
        .map(String::from)
        .collect();

    let got = unverifiable_keys(&baseline, &verifiable);
    assert_eq!(
        got, baseline,
        "金丝雀失败：基线键本轮一个可判定变异都没有，却没被判红"
    );

    // 反例：同键本轮有可判定结果（哪怕仍是 missed）就不该报——否则门会把存量债重复计。
    assert!(
        unverifiable_keys(&baseline, &baseline).is_empty(),
        "金丝雀失败：可验证的键被误报"
    );
    // 空基线不该产红（新仓第一录时没有既有债可核）。
    assert!(unverifiable_keys(&[], &verifiable).is_empty());
    // 旧判据仍在位：新增 missed 依然报。
    assert_eq!(new_missed(&["a::b".into()], &[]), vec!["a::b".to_string()]);
}

/// 从期望文件里取 `SECTION` 段下的键（期望文件由 cargo-mutants 真实产物生成）。
fn expected_section(text: &str, section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') && !line.contains("::") {
            inside = line.trim() == section;
            continue;
        }
        if inside {
            out.push(line.trim().to_string());
        }
    }
    out.sort();
    out
}

#[test]
fn mutants_key_extraction_matches_cargo_mutants_real_output() {
    // 语料 = cargo-mutants 27 在一整轮真实运行里的 outcomes 切片（102 条，含
    // Missed/Caught/Unviable 三类）；期望值由该产物自己的 summary 字段导出，
    // 不是门作者按"我以为键长什么样"手写。file::function 的顺序、
    // `<impl T for X>::method` 这种函数名形态、Unviable 不该进可验证集，都在这里钉住。
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data");
    let raw = std::fs::read_to_string(dir.join("outcomes-sample.json"))
        .expect("读变异语料（xtask/tests/data/outcomes-sample.json）");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");
    let want_raw = std::fs::read_to_string(dir.join("outcomes-sample.expected.txt"))
        .expect("读期望（xtask/tests/data/outcomes-sample.expected.txt）");

    let want_missed = expected_section(&want_raw, "MISSED");
    let want_verifiable = expected_section(&want_raw, "VERIFIABLE");
    assert!(
        want_missed.len() >= 10 && want_verifiable.len() > want_missed.len(),
        "语料退化（期望 missed {} 键、可验证 {} 键）——期望为空会让本测试假绿",
        want_missed.len(),
        want_verifiable.len()
    );

    let got_missed = keys_by_summary(&parsed, |s| s == "MissedMutant").expect("取 missed 键");
    assert_eq!(
        got_missed, want_missed,
        "未捕获键提取与工具产物不一致（键形态：file::function）"
    );
    let got_verifiable = keys_by_summary(&parsed, |s| {
        ["CaughtMutant", "MissedMutant", "TimeoutMutant"].contains(&s)
    })
    .expect("取可验证键");
    assert_eq!(
        got_verifiable, want_verifiable,
        "可验证键集应含 caught+missed+timeout、排除全 unviable 的键"
    );
    // 可验证判据在这份真语料上的实际效果：基线取可验证集之外还含一个"本轮没判定"的旧键
    let mut baseline = got_verifiable.clone();
    baseline.push("xtask/src/mutants.rs::missed_key".to_string());
    let lost = unverifiable_keys(&baseline, &got_verifiable);
    assert_eq!(
        lost,
        vec!["xtask/src/mutants.rs::missed_key".to_string()],
        "旧键 missed_key 在本轮无任何可判定变异，必须被点名"
    );
}

#[test]
fn canary_disk_full_in_unviable_log_is_named_while_compile_error_stays_clear() {
    // 语料见 xtask/tests/data/disk-corpus/README.md：三条都是真产物逐字切片，
    // 只有盘满那条的日志是重建（原始文件被下一轮覆盖，签名文本有账可查）。
    // 语料在仓里摊平存、用 .txt 后缀、测试期拼进临时目录，绕开两条实测到的坑：
    // ① cargo-mutants 的 copy_tree.rs:109 无条件跳过名为 `mutants.out` 的目录，随仓建
    //    这层目录会让 scratch 树里缺语料（实测：未变异树自测即失败，门如实中止）；
    // ② 本仓 .gitignore 第 6 行的 *.log 不收日志文件，用 .log 存语料等于没提交，
    //    CI 上 cargo test --workspace 会缺文件。
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/disk-corpus");
    let raw = std::fs::read_to_string(corpus.join("outcomes.json")).expect("读盘满语料");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");

    let out_dir = std::env::temp_dir().join(format!(
        "adv-disk-corpus-{}-{}",
        std::process::id(),
        file!().replace(['/', '.', '\\'], "_")
    ));
    let log_dir = out_dir.join("mutants.out").join("log");
    std::fs::create_dir_all(&log_dir).expect("建临时产物面");
    let enospc = log_dir.join("reconstructed-2026-10-05-enospc.log");
    let compile = log_dir.join("xtask__src__mutants.rs_line_180_col_5.log");
    std::fs::copy(corpus.join("enospc.txt"), &enospc).expect("摆盘满语料日志");
    std::fs::copy(corpus.join("compile-error.txt"), &compile).expect("摆对照日志");

    // 退化守卫：日志里真有/真没有签名，否则判据"读不到就跳过"会让本测试空转。
    let enospc_text = std::fs::read_to_string(&enospc).expect("读盘满日志");
    let compile_text = std::fs::read_to_string(&compile).expect("读对照日志");
    assert!(
        disk_failure_signature(&enospc_text).is_some(),
        "语料退化：盘满日志里已找不到签名"
    );
    assert_eq!(
        disk_failure_signature(&compile_text),
        None,
        "对照退化：真编译错误日志里混进了盘满签名"
    );

    let hits = unviable_disk_failures(&parsed, &out_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
    assert_eq!(
        hits.len(),
        1,
        "只该点名盘满那条（另两条：真编译错误 + caught）：{hits:?}"
    );
    assert!(
        hits[0].contains("crates/adv-cli/src/main.rs::engine_of"),
        "没点名到盘满那条的键：{}",
        hits[0]
    );
    assert!(
        hits[0].contains("No space left on device"),
        "报错要带上命中的原话：{}",
        hits[0]
    );
    assert!(
        hits[0].contains("mutants.out"),
        "报错要能让人顺路找到日志：{}",
        hits[0]
    );
}

#[test]
fn canary_scratch_precheck_only_reds_on_measured_shortfall() {
    const GIB: u64 = 1024 * 1024 * 1024;
    // 查不到 = 如实播报"未知"，不拿"查不到"当"没空间"；真判据是产物日志签名。
    assert!(
        !scratch_is_short(None, 8),
        "金丝雀失败：查不到余量就被判红了"
    );
    assert!(
        scratch_is_short(Some(7 * GIB), 8),
        "金丝雀失败：低于下限没判红"
    );
    assert!(
        !scratch_is_short(Some(8 * GIB), 8),
        "边界：等于下限不算不足（判红条件写成 <= 就在此处暴露）"
    );
    assert!(
        scratch_is_short(Some(8 * GIB - 1), 8),
        "边界：低于 1 字节就该判红"
    );
    assert_eq!(
        min_free_gib(None),
        8,
        "默认下限变了，账里的两个观测点就不成立"
    );
    assert_eq!(min_free_gib(Some("20".into())), 20);
    assert_eq!(min_free_gib(Some(" 12 ".into())), 12);
    assert_eq!(
        min_free_gib(Some("abc".into())),
        8,
        "坏值该回退默认而不是放行/崩"
    );
    assert_eq!(
        min_free_gib(Some("0".into())),
        8,
        "0 会让预检永不判红，必须回退"
    );
    assert_eq!(human_bytes(Some(GIB + GIB / 2)), "1.5GB");
    assert_eq!(human_bytes(None), "未知");
}

#[test]
fn canary_scratch_space_query_actually_answers_on_this_machine() {
    assert_eq!(
        drive_letter(std::path::Path::new(r"C:\Users\x\AppData\Local\Temp")).as_deref(),
        Some("C"),
        "取不到盘符就只能退回 df，而 Windows 上 df 不保证在 PATH"
    );
    assert_eq!(
        drive_letter(std::path::Path::new(r"d:\tmp")).as_deref(),
        Some("D"),
        "小写盘符要归一（u8 直接 to_string 会给 ASCII 码 68，实测踩过）"
    );
    // 边界：不满足"字母 + 冒号"的一律判无盘符，不许误取
    assert_eq!(drive_letter(std::path::Path::new("C")), None);
    assert_eq!(drive_letter(std::path::Path::new("1:x")), None);
    assert_eq!(drive_letter(std::path::Path::new("CX")), None);
    assert_eq!(drive_letter(std::path::Path::new("/tmp")), None);

    // 预检对象 = cargo-mutants 自己那个 temp_dir（TMP/TEMP 由环境继承给子进程），
    // 所以这里查的就是它写 scratch 的盘。
    let temp = std::env::temp_dir();
    assert!(
        temp.is_dir(),
        "临时目录 {} 不存在，预检与 cargo-mutants 会看同一个不存在的目录",
        temp.display()
    );
    let (free, channel) = free_bytes_probe(&temp);
    assert_ne!(channel, "none", "金丝雀失败：两条查询通道都没给出数");
    let bytes = free.expect("查临时盘余量没拿到数——盘量预检在这台机器上是空装的");
    assert!(
        bytes > 100 * 1024 * 1024,
        "余量读数 {bytes} 字节不像真实盘量（单位换算错了？）"
    );
    #[cfg(windows)]
    {
        // 带盘符必须走 PowerShell 通道：把 drive_letter 改成恒 None 会在这里红
        // （MSYS 的 df 会吞掉反斜杠参数，实测把 `C:\...\Temp` 报成 /tmp 那个挂载）。
        assert_eq!(
            channel, "powershell",
            "Windows 上带盘符的路径没走 PowerShell 通道"
        );
        assert_eq!(
            free_bytes_probe(std::path::Path::new(r"Z:\definitely-not-here")).1,
            "none",
            "不存在的盘该报查不到，不是给个猜出来的数"
        );
    }
}

#[test]
fn canary_disk_free_parsers_agree_with_real_command_output() {
    // 语料 = 2026-10-06 本机 `df -kP` 与 `powershell Get-PSDrive` 的逐字输出。
    // 两把尺查同一块盘，读数必须对到字节——单位换算（1K 块 × 1024）或取列写错就红。
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let two = std::fs::read_to_string(dir.join("df-kp-two.txt")).expect("读 df 双行语料");
    let single = std::fs::read_to_string(dir.join("df-kp-single.txt")).expect("读 df 单行语料");
    let ps = std::fs::read(dir.join("powershell-free.txt")).expect("读 PowerShell 语料");

    assert_eq!(
        parse_df_avail(&single),
        Some(42_853_012 * 1024),
        "单行 df 输出取错了列或换算错了单位"
    );
    assert_eq!(
        parse_df_avail(&two),
        Some(38_017_224 * 1024),
        "多文件系统时该取最后一行（这份语料的最后一行是 D:）"
    );
    assert_eq!(parse_u64_lines(&ps), Some(43_881_484_288));
    assert_eq!(
        parse_df_avail(&single),
        parse_u64_lines(&ps),
        "同一块盘两把尺读数不一致（语料同日实测，差值只可能是单位/列号错）"
    );
    // 畸形与退化输入一律 None：宁可"未知"也不猜
    assert_eq!(
        parse_df_avail(&String::from_utf8_lossy(&ps)),
        None,
        "非 df 文本不该被解析出数"
    );
    assert_eq!(parse_df_avail(""), None);
    assert_eq!(
        parse_df_avail("Filesystem 1024-blocks Used Available Capacity Mounted on\n"),
        None,
        "只有表头时不能把列名当数字"
    );
    assert_eq!(
        parse_u64_lines(two.as_bytes()),
        None,
        "df 表里的数字不是 PowerShell 的单值输出"
    );
    assert_eq!(parse_u64_lines(b""), None);
    assert_eq!(parse_u64_lines("错误：找不到驱动器\r\n".as_bytes()), None);
    assert_eq!(
        parse_u64_lines(b"\r\n  12345 \r\n678\r\n"),
        Some(12_345),
        "该取第一个纯数字行，且要吃得下 CRLF 与行首尾空白"
    );
}

#[test]
fn canary_disk_query_args_and_short_message_are_pinned() {
    assert_eq!(
        powershell_args("C"),
        vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "(Get-PSDrive -Name C).Free".to_string(),
        ],
        "PowerShell 参数形态变了就等于换了查询方式，得先红在测试里"
    );
    assert_eq!(
        powershell_args("d").last(),
        Some(&"(Get-PSDrive -Name d).Free".to_string())
    );
    assert_eq!(
        df_args(std::path::Path::new("/tmp")),
        vec!["-kP".to_string(), "/tmp".to_string()],
        "-P（POSIX 输出格式）丢了就会在多列名时取错列"
    );

    const GIB: u64 = 1024 * 1024 * 1024;
    let tmp = std::path::Path::new(r"C:\Users\x\AppData\Local\Temp");
    assert_eq!(
        short_message(tmp, Some(9 * GIB), 8),
        None,
        "余量够不该出文案"
    );
    assert_eq!(
        short_message(tmp, None, 8),
        None,
        "查不到余量不该出盘满文案"
    );
    let msg = short_message(std::path::Path::new(r"C:\T"), Some(GIB + GIB / 2), 8)
        .expect("低于下限必须有文案");
    assert!(msg.contains("1.5GB < 8GB"), "读数与下限都要写进文案：{msg}");
    assert!(msg.contains("C:\\T"), "要点名是哪个目录：{msg}");
    assert!(
        msg.contains("TMP=D:/tmp/adv-mut"),
        "文案里必须带可照抄的一行修法：{msg}"
    );
}

#[test]
fn canary_tally_counts_match_raw_product_text() {
    // 期望值不手打：同一份真语料（102 条）按原始文本里的 summary 字样独立数一遍，
    // 与 tally 走 serde 的结果对账——只改 tally 的判定串或漏一类都会红。
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/outcomes-sample.json");
    let raw = std::fs::read_to_string(&path).expect("读变异语料");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");
    let counted = |s: &str| raw.matches(&format!("\"summary\": \"{s}\"")).count();
    let [total, caught, missed, unviable] = tally(&parsed);
    assert_eq!(total, 102, "语料条数变了，下面三个数就得重新对");
    assert_eq!(
        [caught, missed, unviable],
        [
            counted("CaughtMutant"),
            counted("MissedMutant"),
            counted("Unviable")
        ],
        "计数与产物文本不符（caught/missed/unviable 任一位数错都会暴露）"
    );
    assert_eq!(
        caught + missed + unviable,
        total,
        "语料里只有这三类，加起来必须等于总数"
    );
    assert!(
        caught > missed && unviable > 0,
        "语料退化：分布塌了就先修语料"
    );
}
