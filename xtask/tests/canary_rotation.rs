//! 金丝雀（变异门 `xtask mutants --rotate` 轮转档）：窗口选取、面漂移、关账两条判据、
//! 预算投影、游标并集/落盘逐个证明会红。
//!
//! 为什么这一档尤其需要单测：一次真跑是几十分钟到几小时，"这一圈顶一轮全档"这句话
//! 如果只能靠跑满一圈来验，就等于没验（DD-0017 登记的就是这种状态）。

use xtask::mutants::RoundEvidence;
use xtask::rotation::{
    self, Window, face_drift, face_total, file_counts, file_gaps, key_gap_files, key_gaps,
    listing_file, load_lap, open_lap, pick_window, progress_line, record, refuse_partial_update,
    save_lap, uncovered, window_budget, window_scratch_message,
};

/// 真产物片段（2026-10-10 `cargo mutants --list --in-diff` 原样抄的三行 + 一行 cargo 噪声）。
const REAL_LISTING: &str = "crates/adv-ast-rust/src/main.rs:67:9: replace MirProbe::harvest with ()\n\
crates/adv-index/src/bm25.rs:19:5: replace tokenize -> Vec<String> with vec![]\n\
crates/adv-index/src/bm25.rs:20:13: delete match arm Operand::Constant(konst) in callee_path\n\
Found 3 mutants\n";

fn face() -> Vec<(String, usize)> {
    file_counts(REAL_LISTING)
}

fn ev(verifiable: &[&str], missed: &[&str], resource: &[&str]) -> RoundEvidence {
    RoundEvidence {
        total: verifiable.len(),
        missed: missed.iter().map(|s| s.to_string()).collect(),
        verifiable: verifiable.iter().map(|s| s.to_string()).collect(),
        resource: resource.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn listing_only_counts_real_mutant_lines() {
    // 形状：`file:line:col: 描述`。描述里再出现冒号（`-> &'tcx mir::Body<'tcx>`、
    // `Operand::Constant`）不能把文件认丢；cargo 自己的提示行不能算变异。
    assert_eq!(
        listing_file(
            "crates/adv-index/src/bm25.rs:19:5: replace tokenize -> Vec<String> with vec![]"
        ),
        Some("crates/adv-index/src/bm25.rs".to_string()),
        "真产物行必须认出来"
    );
    assert_eq!(
        listing_file("xtask/src/mir.rs:114:5: replace mir_body -> &'tcx mir::Body<'tcx> with x"),
        Some("xtask/src/mir.rs".to_string()),
        "描述里带冒号也要认对文件"
    );
    assert_eq!(
        listing_file("Found 3 mutants"),
        None,
        "不是 listing 行就别编出文件"
    );
    assert_eq!(listing_file("warning: unused"), None);
    assert_eq!(
        listing_file("a.rs:not-a-line:3: x"),
        None,
        "行号列号不是数字就不算"
    );

    // 计数按文件聚合并排序：窗口选取的确定性建立在它上面。
    assert_eq!(
        face(),
        vec![
            ("crates/adv-ast-rust/src/main.rs".to_string(), 1),
            ("crates/adv-index/src/bm25.rs".to_string(), 2),
        ],
        "噪声行不计入，按文件名排序"
    );
}

#[test]
fn window_greedy_respects_budget_and_never_deadlocks() {
    let face = vec![
        ("a.rs".to_string(), 10usize),
        ("b.rs".to_string(), 20usize),
        ("c.rs".to_string(), 50usize),
    ];
    let w = pick_window(&face, &[], 30);
    assert_eq!(w.files, ["a.rs", "b.rs"], "贪心取到预算为止");
    assert_eq!(w.mutants, 30);
    assert_eq!(w.over_budget, None);

    // 第二个窗：已进窗的文件不再进（这是"一圈不重不漏"的机械保证）。
    let covered: Vec<String> = w.files.clone();
    let next = pick_window(&face, &covered, 30);
    assert_eq!(next.files, ["c.rs"]);

    // c.rs 自己就超预算：必须照样开窗并点名，否则它永远进不了圈 ⇒ 圈永远关不了账。
    let fat = pick_window(&face, &["a.rs".to_string(), "b.rs".to_string()], 5);
    assert_eq!(fat.files, ["c.rs"], "单文件超预算也要开窗（防死锁）");
    let why = fat.over_budget.expect("超预算必须点名");
    assert!(
        why.contains("c.rs") && why.contains("50"),
        "文案要带文件与条数：{why}"
    );

    // 全覆盖之后没有窗可开。
    let done = pick_window(&face, &["a.rs".into(), "b.rs".into(), "c.rs".into()], 100);
    assert!(done.files.is_empty() && done.mutants == 0);
    assert_eq!(uncovered(&face, &[]).len(), 3);
}

#[test]
fn face_drift_names_both_sides() {
    let recorded = vec![("a.rs".to_string(), 10usize)];
    assert_eq!(face_drift(&recorded, &recorded), None, "面没变就别喊变");
    let live = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 4usize)];
    let why = face_drift(&recorded, &live).expect("面在圈中变了必须点名");
    assert!(why.contains("1 文件/10 条"), "定格侧的数要摆出来：{why}");
    assert!(why.contains("2 文件/14 条"), "当前侧的数要摆出来：{why}");
    assert!(why.contains("重开一圈"), "要说清后果：{why}");
    assert_eq!(face_total(&live), 14);
}

#[test]
fn closure_needs_every_file_and_every_baseline_key() {
    let face = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 20usize)];
    let mut lap = open_lap("main...HEAD", 30, &face, 0);
    let window = Window {
        files: vec!["a.rs".to_string()],
        mutants: 10,
        over_budget: None,
    };
    record(&mut lap, &window, &mut ev(&["a.rs::f"], &[], &[]));

    // 关账条件①：面里还有文件没进窗。
    assert_eq!(file_gaps(&face, &lap.covered_files), ["b.rs"]);
    assert!(
        key_gaps(&["a.rs::f".to_string()], &face, &lap).is_empty(),
        "已判过的键不该再算缺口"
    );

    // 关账条件②：键在面上但那扇窗里没判到 ⇒ 说"查 unviable 日志"。
    let gaps = key_gaps(&["b.rs::g".to_string()], &face, &lap);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].contains("unviable"),
        "在面上却没判到，要指到那扇窗的日志：{}",
        gaps[0]
    );

    // 键压根不在面上 ⇒ 另一种成因（改口径或划账），别和上面混成一条。
    let gaps = key_gaps(&["z.rs::h".to_string()], &face, &lap);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].contains("不在变异面上"),
        "全档口径也取不到它，要说清：{}",
        gaps[0]
    );

    // 函数段自带 `::` 的键（真基线里有 SearchIndex::search 这种）也要归对成因：
    // 文件在面上就按"进了窗没判到"说——从右切错位会把成因说反（金丝雀首跑抓到过）。
    let gaps = key_gaps(&["b.rs::SearchIndex::g".to_string()], &face, &lap);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].contains("键缺口专用窗") && !gaps[0].contains("不在变异面上"),
        "多段函数名的键文件段切错会把成因说反：{}",
        gaps[0]
    );

    // 进度行把两个数都摆出来（不让人自己数）。
    let line = progress_line(&face, &lap, &["a.rs::f".to_string(), "b.rs::g".to_string()]);
    assert!(line.contains("1/2 文件"), "{line}");
    assert!(line.contains("已判 1/2"), "{line}");
    assert!(line.contains("未关账"), "{line}");
}

#[test]
fn record_unions_are_sorted_and_resource_rounds_counted() {
    let face = vec![("a.rs".to_string(), 10usize)];
    let mut lap = open_lap("main...HEAD", 30, &face, 0);
    let window = Window {
        files: vec!["a.rs".to_string()],
        mutants: 10,
        over_budget: None,
    };
    // 乱序喂进去：游标要归一成同一份 JSON（重放可核对的前提）。
    record(
        &mut lap,
        &window,
        &mut ev(
            &["a.rs::z", "a.rs::a", "a.rs::z"],
            &["a.rs::z"],
            &["unviable 是盘满造成的缺测"],
        ),
    );
    assert_eq!(lap.covered_keys, ["a.rs::a", "a.rs::z"], "并集要排序去重");
    assert_eq!(lap.missed_keys, ["a.rs::z"]);
    assert_eq!(lap.covered_files, ["a.rs"]);
    assert_eq!(lap.rounds, 1);
    assert_eq!(
        lap.resource_rounds, 1,
        "资源缺测的窗要计入，不然圈能带着缺测关账"
    );

    record(&mut lap, &window, &mut ev(&["a.rs::m"], &[], &[]));
    assert_eq!(lap.rounds, 2);
    assert_eq!(lap.resource_rounds, 1, "没缺测的窗不该再加");
    assert_eq!(lap.covered_keys, ["a.rs::a", "a.rs::m", "a.rs::z"]);
    // 同一份证据喂两遍不改变键集（幂等），但窗数照实累计。
    record(&mut lap, &window, &mut ev(&["a.rs::m"], &[], &[]));
    assert_eq!(lap.covered_keys, ["a.rs::a", "a.rs::m", "a.rs::z"]);
}

#[test]
fn partial_lap_cannot_rerecord_the_baseline() {
    // 未关账 + --update = 拒绝：拿半圈的 missed 整表替换会删掉别的窗里才有的键。
    let why = refuse_partial_update(false, true).expect("未关账时 update 必须被拒");
    assert!(why.contains("不许在未关账时重录基线"), "{why}");
    assert_eq!(
        refuse_partial_update(true, true),
        None,
        "关账那一轮才有资格重录"
    );
    assert_eq!(refuse_partial_update(false, false), None, "不重录就别拦路");
}

#[test]
fn budget_and_scratch_projection_fail_the_way_they_should() {
    assert_eq!(window_budget(None), (rotation::DEFAULT_WINDOW_BUDGET, None));
    assert_eq!(window_budget(Some("90")), (90, None));
    let (budget, warn) = window_budget(Some("0"));
    assert_eq!(budget, rotation::DEFAULT_WINDOW_BUDGET);
    assert!(
        warn.expect("坏值要点名")
            .contains("ADV_MUTANTS_WINDOW_BUDGET")
    );
    let (budget, warn) = window_budget(Some("abc"));
    assert_eq!(budget, rotation::DEFAULT_WINDOW_BUDGET);
    assert!(warn.is_some(), "非数字也要点名，不能静默");

    let gib = 1024u64 * 1024 * 1024;
    // 默认预算（150 条 × 16MB 投影 = 2.34GB）在正常盘上不该被拦住。
    assert_eq!(
        window_scratch_message(Some(22 * gib), rotation::DEFAULT_WINDOW_BUDGET, "df"),
        None,
        "日常窗口就别被这条拦住（拦住就等于没人跑得起轮转）"
    );
    // 查不到余量 ⇒ 不判红（与全档那条预检同一口径：不把"查不到"当成"没空间"）。
    assert_eq!(window_scratch_message(None, 10, "none"), None);
    // 病态预算（1000 条 ⇒ 投影 15.6GB）⇒ 当面拒绝，并把投影/实测/建议预算摆出来。
    let why = window_scratch_message(Some(2 * gib), 1000, "df").expect("投影放不下必须拦");
    assert!(why.contains("本窗 1000 条"), "{why}");
    assert!(why.contains("实测只剩 2GB"), "{why}");
    assert!(
        why.contains("ADV_MUTANTS_WINDOW_BUDGET"),
        "要给一行能抄的修法：{why}"
    );
}

#[test]
fn cursor_round_trips_and_keeps_closed_laps() {
    let root =
        std::env::temp_dir().join(format!("adv-rotation-{}-{}", std::process::id(), file!()));
    std::fs::create_dir_all(&root).expect("建临时根");
    let path = root.join("mutants-rotation.json");
    let face = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 5usize)];
    let lap = open_lap("main...HEAD", 30, &face, 2);
    save_lap(&path, &lap).expect("写游标");
    let raw = std::fs::read_to_string(&path).expect("读游标");
    assert!(raw.contains("\"completed_laps\": 2"), "{raw}");
    assert!(
        raw.contains("\"spec\": \"main...HEAD\""),
        "口径要写进游标：{raw}"
    );
    let loaded = rotation::load_lap(&path)
        .expect("读游标不报错")
        .expect("有游标");
    assert_eq!(loaded, lap, "游标必须原样回来（并集/面/计数一个不能丢）");
    // 关账重开一圈：已完成圈数保留，证据清零。
    let reopened = open_lap("main...HEAD", 30, &face, loaded.completed_laps + 1);
    assert_eq!(reopened.completed_laps, 3);
    assert!(reopened.covered_keys.is_empty() && reopened.rounds == 0);
    assert_eq!(reopened.face, face, "面是新的一圈自己的定格面");
    // 游标不存在 = 还没开过圈（不是红）。
    assert!(
        rotation::load_lap(&root.join("nope.json"))
            .expect("读不到不该报错")
            .is_none(),
        "缺文件应当当\"还没开圈\"，不是\"判不了\""
    );
    // 坏 JSON 一律红，不猜。
    std::fs::write(&path, "{ not json").expect("写坏游标");
    assert!(
        rotation::load_lap(&path).is_err(),
        "游标坏了要判不了，不能悄悄重开一圈把证据丢了"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn verdict_line_carries_the_window_so_green_cannot_be_read_as_full_tier() {
    let name = rotation::verdict_name(60);
    assert!(name.contains("轮转档"), "档名要点名这是窗口：{name}");
    assert!(name.contains("60"), "预算要进档名：{name}");
    assert!(name.contains("关账"), "要写清什么才算关账：{name}");
}

#[test]
fn key_gap_files_maps_unjudged_keys_to_their_files_sorted_deduped() {
    // 键的 fn 段自带 `::`（SearchIndex::search、<impl ...>::method）⇒ 从右取第一处切，
    // 别把文件切错；已判过的键不再进窗；同文件多键去重；输出有序（确定性）。
    let mut lap = open_lap("main...HEAD", 150, &face(), 0);
    lap.covered_keys = vec!["crates/adv-index/src/bm25.rs::push_token".to_string()];
    let baseline = vec![
        "crates/adv-rules/src/taint.rs::body_touches_source".to_string(),
        "crates/adv-index/src/bm25.rs::SearchIndex::search".to_string(),
        "crates/adv-index/src/bm25.rs::push_token".to_string(),
        "crates/adv-index/src/bm25.rs::tokenize".to_string(),
        "xtask/src/mir.rs::rustc_print".to_string(),
    ];
    assert_eq!(
        key_gap_files(&baseline, &lap),
        vec![
            "crates/adv-index/src/bm25.rs".to_string(),
            "crates/adv-rules/src/taint.rs".to_string(),
            "xtask/src/mir.rs".to_string(),
        ]
    );
}

#[test]
fn cursor_without_key_gap_rounds_field_loads_as_zero() {
    // 旧游标（bb997cdb 那一轮写盘的形状）没有 key_gap_rounds ⇒ 默认 0，不许读挂：
    // 轮转档升级不能把已在跑的一圈判成"游标坏了"。
    let root = std::env::temp_dir().join(format!("adv-rot-compat-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("cursor.json");
    std::fs::write(
        &path,
        r#"{"spec":"origin/main...HEAD","budget":150,"face":[["a.rs",3]],"covered_files":["a.rs"],"covered_keys":["a.rs::f"],"missed_keys":[],"rounds":1,"resource_rounds":0,"completed_laps":0}"#,
    )
    .expect("写旧形状游标");
    let lap = load_lap(&path)
        .expect("旧游标要能读")
        .expect("旧游标要算已开圈");
    assert_eq!(lap.key_gap_rounds, 0, "缺席字段读成 0，不是报错");
    assert_eq!(lap.rounds, 1);
    let _ = std::fs::remove_dir_all(&root);
}
