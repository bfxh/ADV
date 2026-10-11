//! 金丝雀（变异门 `xtask mutants`）：DD-0018 分诊新增面——生产闭包读数、skip 判定、
//! `--update` 重录通道、档名。跑门时整片判定都在不可 mock 的子进程壳里，
//! 所以把读数/判定面抽成同包纯函数后在这里钉住。

use xtask::mutants::{parse_outcomes, patch_is_empty, tier_label, write_baseline};

/// DD-0018 分诊：三个生产闭包原本零覆盖（测试直接调 `keys_by_summary`，生产侧那三个闭包
/// 没人跑过）⇒ `parse_outcomes` 用**真语料**把读数、unviable 提示、超时点名三面都钉住。
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
fn canary_parse_outcomes_readout_from_real_corpus() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let raw =
        std::fs::read_to_string(dir.join("outcomes-sample.json")).expect("读真语料（102 条）");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");
    let want_raw =
        std::fs::read_to_string(dir.join("outcomes-sample.expected.txt")).expect("读期望文件");

    let (ev, notes) = parse_outcomes(&parsed, std::path::Path::new("OUT"), "轮转").expect("读数");
    // 判定行的数来自 tally，期望值不手打：与语料原文里的 summary 字样对账（见 tally 那条）。
    let counted = |s: &str| raw.matches(&format!("\"summary\": \"{s}\"")).count();
    assert!(
        notes[0].contains("总=102"),
        "读数行的总数与语料不符：{}",
        notes[0]
    );
    assert!(
        notes[0].contains(&format!("捕获={}", counted("CaughtMutant"))),
        "捕获数与语料不符：{}",
        notes[0]
    );
    assert!(
        notes[0].contains(&format!("未捕获={}", counted("MissedMutant"))),
        "未捕获数与语料不符：{}",
        notes[0]
    );
    assert!(notes[0].contains("档=轮转"), "档名要进判定行：{}", notes[0]);
    // 真语料有 26 条 unviable ⇒ 提示必须在（`>0` 写成 `==0` 就会静默少这一行）。
    assert!(
        notes
            .iter()
            .any(|n| n.contains(&format!("unviable {} 条", counted("Unviable")))),
        "有 unviable 却没提示（把缺测读成没问题）：{notes:?}"
    );
    // missed / verifiable 集合与既有期望文件逐键一致。
    assert_eq!(
        ev.missed,
        expected_section(&want_raw, "MISSED"),
        "生产闭包抽出的 missed 键集与产物期望不一致"
    );
    assert_eq!(
        ev.verifiable,
        expected_section(&want_raw, "VERIFIABLE"),
        "生产闭包抽出的可验证键集与产物期望不一致"
    );
    assert!(
        ev.resource.is_empty(),
        "这份语料的 unviable 日志不在仓里，不该报资源缺测：{:?}",
        ev.resource
    );
}

/// 真语料里没有 unviable 的那一面（3 条真 CaughtMutant）⇒ 提示行必须**不出现**。
/// `>0` 写成 `>=0` 时，真有 unviable 的那份仍然照提示，所以只有零这一面能照出来。
#[test]
fn canary_parse_outcomes_stays_quiet_when_nothing_unviable() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/outcomes-clean-sample.json");
    let raw = std::fs::read_to_string(&path).expect("读干净语料（3 条真 CaughtMutant）");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");
    assert!(
        !raw.contains("\"summary\": \"Unviable\""),
        "语料退化：这份该是零 unviable 的"
    );
    let (ev, notes) = parse_outcomes(&parsed, std::path::Path::new("OUT"), "全档").expect("读数");
    assert_eq!(notes.len(), 1, "零 unviable ⇒ 只该有判定行：{notes:?}");
    assert!(ev.resource.is_empty());
    assert_eq!(ev.missed, Vec::<String>::new(), "全是捕获 ⇒ missed 空");
    // 键是**去重**的集合：这份语料 3 条变异落在 2 个键上（`main` 里有两条变异）。
    assert_eq!(
        ev.verifiable,
        vec![
            "xtask/src/main.rs::main".to_string(),
            "xtask/src/main.rs::workspace_root".to_string(),
        ],
        "可验证键集要去重且有序（同一函数的多条变异只算一个键）"
    );
}

/// 超时点名面：语料是 2026-10-11 诊断跑里 `seg_eq` 的**真** Timeout 条目
/// （那两条 `+= → *=` 死循环变异触发的，2026-10-06 起 exit 3 就靠这条文案点名）。
#[test]
fn canary_parse_outcomes_names_the_real_timeout_entry() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/outcomes-timeout-sample.json");
    let raw = std::fs::read_to_string(&path).expect("读超时语料（真 Timeout 条目）");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("语料应为 JSON");
    let (ev, notes) = parse_outcomes(&parsed, std::path::Path::new("OUT"), "全档").expect("读数");
    assert_eq!(notes.len(), 2, "一条超时 ⇒ 判定行 + 点名行：{notes:?}");
    let note = &notes[1];
    assert!(note.contains("超时"), "要点明这是超时：{note}");
    assert!(
        note.contains("xtask/src/maturity.rs::seg_eq"),
        "要点名真键（谓词反了就点错键）：{note}"
    );
    assert!(ev.missed.is_empty(), "超时不计入 missed：{:?}", ev.missed);
    assert!(
        ev.verifiable
            .iter()
            .any(|k| k == "xtask/src/maturity.rs::seg_eq"),
        "超时也算真跑到判定环节：{:?}",
        ev.verifiable
    );
}

/// `write_baseline` 是"把现状转正"的唯一入口：不写盘 / 有缺测还照写都必须被抓到。
/// 三个突变（整体换 `Ok(...)`、两个 `delete !`）此前都活着——测试从没碰过它。
#[test]
fn canary_write_baseline_refuses_on_resource_and_replaces_whole_table() {
    let dir = std::env::temp_dir().join(format!("adv-wb-{}-{}", std::process::id(), file!()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    let path = dir.join("mutants-baseline.json");
    std::fs::write(&path, r#"{"missed":["old::key"]}"#).expect("铺一份旧基线");

    // ① 有资源缺测 ⇒ 拒绝重录，文件保持原样（缺测不能当成已清带下去）。
    let refused = write_baseline(&path, &["new::key".to_string()], &["盘满缺测".to_string()])
        .expect("有缺测时不该报错（它是披露通道）");
    assert_eq!(refused, vec!["盘满缺测".to_string()]);
    let raw = std::fs::read_to_string(&path).expect("读回基线");
    assert!(
        raw.contains("old::key") && !raw.contains("new::key"),
        "拒绝重录却动了基线：{raw}"
    );

    // ② 无缺测 ⇒ 整表替换：旧键离账、新键入账。
    let dropped = write_baseline(&path, &["new::key".to_string()], &[]).expect("干净重录");
    assert!(dropped.is_empty(), "干净重录不该返回缺测：{dropped:?}");
    let raw = std::fs::read_to_string(&path).expect("读回基线");
    assert!(raw.contains("new::key"), "新键必须落盘：{raw}");
    assert!(
        !raw.contains("old::key"),
        "整表替换：离账的旧键不许留下：{raw}"
    );

    // ③ 空基线也是一次合法重录（不是"什么都没干"）。
    let path2 = dir.join("fresh.json");
    assert!(
        write_baseline(&path2, &[], &[])
            .expect("空表也要写得出去")
            .is_empty()
    );
    assert!(
        path2.is_file(),
        "空表重录也要落盘（否则 `--update` 静默无效）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn canary_patch_is_empty_pins_both_directions() {
    // `delete !` 会让门在**有** diff 时报 skip —— 那是假绿面（整轮不产变异仍算过）。
    assert!(patch_is_empty(""));
    assert!(patch_is_empty("   \n\t\r\n"), "纯空白 = 无差异");
    assert!(
        !patch_is_empty("diff --git a/x.rs b/x.rs"),
        "有 diff 就不许报 skip"
    );
}

#[test]
fn canary_tier_label_names_both_tiers() {
    // 档名混进裁决行是"绿在名字里带着档位"的唯一载体；写成空串就看不出是哪一档绿的。
    assert_eq!(tier_label(None), "全档");
    assert_eq!(tier_label(Some("abc1234")), "增量");
}
