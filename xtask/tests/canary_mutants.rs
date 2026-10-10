//! 金丝雀（变异门 `xtask mutants`）：键抽取、可验证标签、退出码、资源签名（盘满/内存）、
//! 盘量预检、并发覆盖逐个证明会红。

use xtask::mutants::{
    Baseline, Resource, VERIFIABLE_SUMMARIES, completed_status, df_args, diff_spec, drive_letter,
    free_bytes_probe, free_bytes_via_df, human_bytes, keys_by_summary, min_free_gib, mutants_jobs,
    new_missed, parse_df_avail, parse_u64_lines, powershell_args, precheck_scratch,
    refuse_incremental_update, resource_signature, scratch_is_short, short_message, tally,
    tier_verdicts, timeout_note, unverifiable_keys, unviable_resource_failures, verdict_name,
};

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

/// 造一条 outcomes 条目：`summary` 用 cargo-mutants 真标签，键为 `file::fn`。
fn outcome(summary: &str, file: &str, fname: &str) -> serde_json::Value {
    serde_json::json!({
        "summary": summary,
        "scenario": { "Mutant": { "file": file, "function": { "function_name": fname } } }
    })
}

#[test]
fn canary_mutants_verifiable_labels_are_the_real_ones() {
    // 标签名写错（曾写成 "TimeoutMutant"）不会报错，只会让超时变异从可验证集里静默消失：
    // 一个键的变异全超时 ⇒ 本该算"本轮可判定"，却被报成不可验证（假红）或反向漏计。
    let parsed = serde_json::json!({ "outcomes": [
        outcome("Timeout", "xtask/src/fake.rs", "only_timeouts"),
        outcome("CaughtMutant", "xtask/src/fake.rs", "killed"),
        outcome("Unviable", "xtask/src/fake.rs", "never_compiled"),
        outcome("MissedMutant", "xtask/src/fake.rs", "survived"),
    ]});
    let verifiable =
        keys_by_summary(&parsed, |s| VERIFIABLE_SUMMARIES.contains(&s)).expect("可验证键集");
    assert!(
        verifiable.contains(&"xtask/src/fake.rs::only_timeouts".to_string()),
        "金丝雀失败：超时也算真跑到判定环节"
    );
    assert!(verifiable.contains(&"xtask/src/fake.rs::killed".to_string()));
    assert!(verifiable.contains(&"xtask/src/fake.rs::survived".to_string()));
    assert!(
        !verifiable.contains(&"xtask/src/fake.rs::never_compiled".to_string()),
        "金丝雀失败：Unviable（没编译/没运行）不该算可验证"
    );
    // 门用的标签集必须与 cargo-mutants 真产物一致——漂了这条会红。
    assert_eq!(
        VERIFIABLE_SUMMARIES.join(","),
        "CaughtMutant,MissedMutant,Timeout"
    );
}

#[test]
fn canary_mutants_reads_exit_3_timeouts_as_a_completed_run() {
    // 上游契约（mutants.rs/exit-codes.html）：0 全捕获 / 2 有未捕获 / 3 有超时都算跑完；
    // 1 用法错 / 4 基线自身就红或挂 / 5 patch 与树不符 / 6 patch 非法 / 70 内部错不算。
    // 旧判据只认 0|2：2026-10-06 两片实测里 `seg_eq` 的两个 `+= → *=` 死循环触发 exit 3，
    // 整道门在棘轮比对**之前**中止，把"门绿"报成了 cargo-mutants 的转储。
    assert!(completed_status(Some(0)));
    assert!(completed_status(Some(2)));
    assert!(
        completed_status(Some(3)),
        "金丝雀失败：把超时当失败 ⇒ 有变异挂住时这道门根本不判"
    );
    for bad in [Some(1), Some(4), Some(5), Some(6), Some(70), None] {
        assert!(!completed_status(bad), "金丝雀失败：{bad:?} 不该被当成跑完");
    }
    // 超时键必须能逐个点名：另一种可能是 timeout 定得太低，那会把存活变异藏在这里。
    let parsed = serde_json::json!({ "outcomes": [
        outcome("Timeout", "xtask/src/x.rs", "hangs"),
        outcome("MissedMutant", "xtask/src/x.rs", "survived"),
    ]});
    let timed_out = keys_by_summary(&parsed, |s| s == "Timeout").expect("超时键集");
    assert_eq!(timed_out, vec!["xtask/src/x.rs::hangs".to_string()]);
    assert_eq!(
        keys_by_summary(&parsed, |s| s == "MissedMutant").expect("missed 键集"),
        vec!["xtask/src/x.rs::survived".to_string()],
        "金丝雀失败：超时与未捕获混成一类"
    );
    // 文案要真点名，且不把超时说成 missed。
    assert_eq!(timeout_note(&[]), None);
    let note = timeout_note(&["xtask/src/x.rs::hangs".to_string()]).expect("有超时该有文案");
    assert!(note.contains("xtask/src/x.rs::hangs"), "{note}");
    assert!(note.contains("不计入 missed"), "{note}");
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
    let (disk_sig, disk_line) =
        resource_signature(&enospc_text).expect("语料退化：盘满日志里已找不到签名");
    assert_eq!(disk_line, Resource::Disk, "盘满语料必须归在磁盘线");
    assert_eq!(disk_sig, "No space left on device", "签名应取实测原话");
    assert_eq!(
        resource_signature(&compile_text),
        None,
        "对照退化：真编译错误日志里混进了资源签名"
    );

    let hits = unviable_resource_failures(&parsed, &out_dir);
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
fn canary_oom_in_baseline_log_is_memory_line_not_disk() {
    // 语料 oom.txt = 2026-10-09 真实一轮未变异基线日志逐字拷贝（vectorscan C++ 编译
    // 阶段 cc1plus OOM），见同目录 README。判据要求：签名命中、归内存线、与盘满
    // 互不污染（盘满签名一条都不该在纯 OOM 日志上误报）。
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/disk-corpus");
    let oom = std::fs::read_to_string(corpus.join("oom.txt")).expect("读 OOM 语料");

    let (sig, line) = resource_signature(&oom).expect("语料退化：OOM 日志里已找不到签名");
    assert_eq!(line, Resource::Memory, "cc1plus OOM 必须归内存线");
    assert_eq!(sig, "out of memory", "签名应取实测原话");

    // 反向：盘满签名集在纯 OOM 日志上零误报（两条资源线互斥的活证据）。
    let disk_only = [
        "No space left on device",
        "final link failed",
        "ENOSPC",
        "os error 112",
    ];
    assert!(
        disk_only.iter().all(|s| !oom.contains(s)),
        "OOM 语料里混进了盘满签名，两条线会互相污染"
    );

    // 重建条目走 outcomes 扫描分支（同一循环只换签名集）：键名如实标注重建来源。
    let rebuilt = serde_json::json!({
        "outcomes": [{
            "scenario": {"Mutant": {"file": "crates/adv-core/src/lib.rs", "function": {"function_name": "rebuilt_oom_entry"}}},
            "summary": "Unviable",
            "log_path": "log/rebuilt-2026-10-09-oom.log"
        }]
    });
    let out_dir = std::env::temp_dir().join(format!(
        "adv-oom-corpus-{}-{}",
        std::process::id(),
        file!().replace(['/', '.', '\\'], "_")
    ));
    let log_dir = out_dir.join("mutants.out").join("log");
    std::fs::create_dir_all(&log_dir).expect("建临时产物面");
    let log = log_dir.join("rebuilt-2026-10-09-oom.log");
    std::fs::write(&log, &oom).expect("摆 OOM 语料日志");
    let hits = unviable_resource_failures(&rebuilt, &out_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
    assert_eq!(hits.len(), 1, "重建的 OOM 条目必须被点名：{hits:?}");
    assert!(hits[0].contains("内存"), "文案要点名资源线：{}", hits[0]);
    assert!(
        hits[0].contains("rebuilt_oom_entry"),
        "没点名到键：{}",
        hits[0]
    );
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

/// 试验面匹配的三条边界：`*` 不跨段、结尾 `/**` 吃任意深度、少一层不算命中。
#[test]
fn canary_df_channel_answers_with_the_number_its_own_output_gives() {
    // DD-0001 的一半：`free_bytes_via_df` 在 Windows 上是备胎通道（powershell 先答），
    // 所以它两条薄壳变异（整体换成 None、删掉 status 的取反）一直没人在看。
    // 这里把"刚跑出来的 df 输出"与"通道函数给的数"钉在一起：不一致就是通道少检查或多检查。
    let dir = std::env::temp_dir();
    let Ok(out) = std::process::Command::new("df")
        .args(df_args(&dir))
        .output()
    else {
        eprintln!("本机没有 df ⇒ 这两条变异在本机不可判（变异门跑在有 df 的机器上）");
        return;
    };
    assert!(out.status.success(), "df 存在却没跑成");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let parsed = parse_df_avail(&text);
    assert!(parsed.is_some(), "df 跑成功但解析拿不出数：{text}");
    // 只比"有没有数"，不比数值：两次 df 之间空闲量会变（本机实测会差几 MB），
    // 数值的对错已经钉在 df-kp-*.txt 语料那条金丝雀上。
    assert!(
        free_bytes_via_df(&dir).is_some(),
        "df 跑成功、语料解析也拿得出数，通道函数却说没有数 ⇒ status 检查或解析被改坏了"
    );
}

#[test]
fn canary_precheck_names_shortfall_and_stays_quiet_when_roomy() {
    // DD-0001 的另一半：`precheck_scratch` 的三条变异（换成 vec![]、vec![String::new()]、
    // vec!["xyzzy"]）在余量充足的机器上看不出来——它只在真要跑门时被调，而那时无下限会红。
    // 用一个不可能的下限（1e9 GiB ≈ 1EB）逼它点名。
    let roomy = precheck_scratch(0);
    assert!(roomy.is_empty(), "下限 0 不该判红：{roomy:?}");
    let scratch = std::env::temp_dir();
    let (free, channel) = free_bytes_probe(&scratch);
    let absurd = precheck_scratch(1_000_000_000);
    match free {
        Some(_) => {
            assert_eq!(
                absurd.len(),
                1,
                "该点名恰好一次，实得 {absurd:?}（通道 {channel}）"
            );
            assert!(
                absurd[0].contains("余量不足"),
                "文案要说清是余量不足：{}",
                absurd[0]
            );
            assert!(
                absurd[0].contains("TMP="),
                "修法要落成能抄的一行：{}",
                absurd[0]
            );
        }
        // 查不到余量时预检必须 fail-open（不判），而不是编一个数——这条与
        // scratch_is_short 的口径一致，写死在这里防"顺手改成缺数即红"。
        None => assert!(
            absurd.is_empty(),
            "测不到余量时必须说'不判'而不是编数：{absurd:?}"
        ),
    }
}

#[test]
fn canary_incremental_tier_cannot_pass_itself_off_as_the_closing_run() {
    // DD-0004：一片里 8 轮全档（单轮 17–40 分钟）里大部分只需要看当轮改动。
    // 但增量档一旦被抄成"收尾绿"就是洗白，所以两条都判死：
    // ① 裁决行自己必须写着"增量档·不作收尾绿"；② 增量档 + --update 直接违规。
    assert_eq!(diff_spec("6201ea4", None), "6201ea4...HEAD");
    assert_eq!(diff_spec("6201ea4", Some("abc1234")), "abc1234..HEAD");
    assert_eq!(verdict_name(None), "mutants");
    let name = verdict_name(Some("abc1234"));
    assert!(name.starts_with("mutants·增量档(abc1234..HEAD"), "{name}");
    assert!(name.contains("不作收尾绿"), "{name}");

    assert_eq!(
        refuse_incremental_update(None, true),
        None,
        "全档重录是正当通道"
    );
    assert_eq!(
        refuse_incremental_update(Some("x"), false),
        None,
        "增量跑但不重录 = 允许"
    );
    let why = refuse_incremental_update(Some("x"), true).expect("增量档重录必须被拒");
    assert!(why.contains("整表替换"), "{why}");
    assert!(why.contains("--since"), "要给出改法：{why}");
}
#[test]
fn canary_incremental_tier_skips_only_the_full_tier_check() {
    // 行为级而不是谓词级：增量档跳过的**只有**"基线键可验证性"这一项；新出现的存活变异
    // 照常判红——否则增量档就成了躲账通道。这条判据以前只能靠一次 17 分钟真跑来验。
    let baseline = Baseline {
        missed: vec!["xtask/src/mir.rs::run".to_string()],
    };
    let verifiable: Vec<String> = vec![]; // 本轮面内一个基线键都没跑到

    let (v_full, note_full) = tier_verdicts(None, &baseline, &[], &verifiable, vec![]);
    assert!(
        v_full.iter().any(|x| x.contains("不可验证")),
        "全档必须把没跑到的基线键判红：{v_full:?}"
    );
    assert_eq!(note_full, None, "全档不该有增量说明行");

    let (v_inc, note_inc) = tier_verdicts(Some("abc1234"), &baseline, &[], &verifiable, vec![]);
    assert!(
        v_inc.iter().all(|x| !x.contains("不可验证")),
        "增量档拿全档的尺量半张面 = 每轮都红的噪声：{v_inc:?}"
    );
    let note = note_inc.expect("增量档要把跳过的东西说出来");
    assert!(note.contains("1 个不在本轮面内"), "{note}");
    assert!(note.contains("轮转窗口关账"), "要指回关账那一轮：{note}");

    // 新增存活变异在增量档里照样判红（整个档位的底线）。
    let (v_new, _) = tier_verdicts(
        Some("abc1234"),
        &baseline,
        &["xtask/src/new.rs::bug".to_string()],
        &["xtask/src/new.rs::bug".to_string()],
        vec![],
    );
    assert_eq!(
        v_new,
        vec!["新增未捕获变异：xtask/src/new.rs::bug".to_string()],
        "增量档放走了新债"
    );
}

#[test]
fn canary_mutants_jobs_cover_is_fail_closed() {
    // 默认 4（16GB 物理内存 + vectorscan 全量编译的实测 OOM 边界）、合法覆盖生效、
    // 0/负/非数字一律退默认并播报——不静默放大并发。
    assert_eq!(mutants_jobs(None), ("4".to_string(), None));
    assert_eq!(mutants_jobs(Some(" 2 ")), ("2".to_string(), None));
    assert_eq!(mutants_jobs(Some("1")).0, "1");
    for bad in ["0", "-2", "x"] {
        let (jobs, note) = mutants_jobs(Some(bad));
        assert_eq!(jobs, "4", "非法值 {bad:?} 必须退回默认 4");
        assert!(note.is_some(), "非法值 {bad:?} 必须播报，不许静默");
    }
}
