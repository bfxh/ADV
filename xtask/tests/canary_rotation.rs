//! 金丝雀（变异门 `xtask mutants --rotate` 轮转档）：窗口选取、面漂移、关账两条判据、
//! 预算投影、游标并集/落盘逐个证明会红。
//!
//! 为什么这一档尤其需要单测：一次真跑是几十分钟到几小时，"这一圈顶一轮全档"这句话
//! 如果只能靠跑满一圈来验，就等于没验（DD-0017 登记的就是这种状态）。

use xtask::mutants::RoundEvidence;
use xtask::rotation::{
    self, Window, closing_note, face_drift, face_total, file_counts, file_gaps, file_limit,
    key_gap_files, key_gaps, listing_file, load_lap, open_lap, pick_window, progress_line, record,
    refuse_partial_update, save_lap, settle_round, uncovered, window_budget,
    window_scratch_message,
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
    // 同 canary.rs：目录名不放 file!()，否则 Windows 下展开成多级目录、只删得掉叶子。
    let root = std::env::temp_dir().join(format!("adv-rotation-{}-cursor", std::process::id()));
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

/// DD-0018 分诊：预算边界的两条都要钉死。`>` 写成 `>=` 会把**正好等于预算**的第二个文件
/// 挤出窗（覆盖变小）；`&&` 写成 `||` 会在只有一个文件已进窗时照样 break（防死锁的那道
/// "至少取 1 个"被反过来用）。
#[test]
fn canary_pick_window_takes_the_file_that_lands_exactly_on_budget() {
    let face = vec![
        ("a.rs".to_string(), 100usize),
        ("b.rs".to_string(), 50usize),
    ];
    let w = pick_window(&face, &[], 150);
    assert_eq!(w.files, ["a.rs", "b.rs"], "正好等于预算的文件要进窗");
    assert_eq!(w.mutants, 150);
    assert_eq!(w.over_budget, None, "没超预算不该点名");

    // 超一丁点就停：1 条之差也不能放进来（否则预算就是软约束，scratch 投影会失真）。
    let tight = pick_window(&face, &[], 149);
    assert_eq!(tight.files, ["a.rs"], "超预算就得停");

    // `over_budget` 点名行自己的边界（134 行，与上面贪心循环是两处分支）：
    // 单文件正好等于预算 = 没超，不许点名（`>` 写成 `>=` 会在这里误报）。
    let exact = pick_window(&[("solo.rs".to_string(), 150usize)], &[], 150);
    assert_eq!(exact.files, ["solo.rs"]);
    assert_eq!(
        exact.over_budget, None,
        "正好等于预算不是超预算（`>=` 突变的症状）"
    );
    // 单文件远低于预算也不许点名（`&&` 写成 `||` 时 `len()==1` 单独成立就误报）。
    let small = pick_window(&[("tiny.rs".to_string(), 10usize)], &[], 150);
    assert_eq!(
        small.over_budget, None,
        "没超预算就别点名（`||` 突变的症状：单文件就报超）"
    );
}

/// 防死锁那条：首个文件自己就超预算时**必须照样开窗**，否则它永远进不了圈、圈永远关不了账。
#[test]
fn canary_pick_window_opens_for_a_single_oversized_first_file() {
    let face = vec![
        ("big.rs".to_string(), 200usize),
        ("b.rs".to_string(), 1usize),
    ];
    let w = pick_window(&face, &[], 150);
    assert_eq!(w.files, ["big.rs"], "单文件超预算也要开窗（防死锁）");
    assert_eq!(w.mutants, 200);
    let why = w.over_budget.expect("超预算必须点名");
    assert!(why.contains("big.rs") && why.contains("200"), "{why}");
    // 第二个文件不跟着进窗（贪心在超预算处 break），并且要点名"点的是哪个文件"——
    // 名字写错就等于让人去查一个没开窗的文件。
    assert!(
        !w.files.contains(&"b.rs".to_string()),
        "超预算的文件不该把后面的拖进窗：{:?}",
        w.files
    );
}

/// `file_limit` 是键缺口专用窗唯一的口径装配：少一个 `--file` 就少判一批键，
/// 而少判在轮转档里表现为"判过了"——所以逐个 flag 都要钉。
#[test]
fn canary_file_limit_repeats_the_flag_per_file() {
    assert_eq!(
        file_limit(&["a.rs".to_string(), "b.rs".to_string()]),
        vec!["--file", "a.rs", "--file", "b.rs"]
    );
    assert_eq!(
        file_limit(&[]),
        Vec::<String>::new(),
        "空清单不许塞半个 flag"
    );
}

/// `closing_note` 三个突变（`>` → `==` / `<` / `>=`）此前全在 println 的格式分支里，
/// 没有任何观察面；提取成纯函数后 0 与 1 两面都能照。
#[test]
fn canary_closing_note_names_the_key_gap_windows_only_when_there_are_some() {
    assert_eq!(closing_note(0), "", "没有键缺口窗就不加尾注");
    assert!(
        closing_note(3).contains("键缺口专用窗 3 扇"),
        "有键缺口窗要点名扇数：{}",
        closing_note(3)
    );
    assert!(
        closing_note(1).contains("diff 面取不到的键"),
        "要说清这批窗判的是什么：{}",
        closing_note(1)
    );
}

/// `progress_line` 的 `==` 反过来时分子分母对调：全判齐的那一圈会显示成"已判 0/2"。
#[test]
fn canary_progress_line_counts_all_judged_keys_when_the_lap_is_complete() {
    let face = vec![("a.rs".to_string(), 1usize), ("b.rs".to_string(), 1usize)];
    let mut lap = open_lap("main...HEAD", 150, &face, 0);
    lap.rounds = 2;
    lap.covered_files = vec!["a.rs".to_string(), "b.rs".to_string()];
    lap.covered_keys = vec!["a.rs::f".to_string(), "b.rs::g".to_string()];
    let baseline = vec!["a.rs::f".to_string(), "b.rs::g".to_string()];
    let line = progress_line(&face, &lap, &baseline);
    assert!(line.contains("2/2 文件"), "{line}");
    assert!(line.contains("已判 2/2"), "{line}");
    assert!(line.contains("（2 窗）"), "窗数要进进度行：{line}");
    // 基线里还有没判的键时不能报满（反向照出分子分母对调）。
    let mut more = baseline.clone();
    more.push("c.rs::h".to_string());
    let partial = progress_line(&face, &lap, &more);
    assert!(partial.contains("已判 2/3"), "{partial}");
}

/// `load_lap` 的 `Err(e) if e.kind() == NotFound` 反过来（guard→true）后，**任何**读失败
/// 都被当成"还没开过圈"⇒ 坏掉的游标会被静默重开一圈、把已判的证据丢掉。
/// 既有那条"坏 JSON"照不到它（坏 JSON 走的是 `Ok(raw)` 分支的解析失败），
/// 所以这里用**读得到但不是文件**（目录）来照：那是读失败且 kind ≠ NotFound。
#[test]
fn canary_load_lap_fails_closed_on_read_errors_other_than_missing() {
    let root = std::env::temp_dir().join(format!("adv-loadlap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");

    // 缺失 = 还没开圈（这条是正常路径，不是红）。
    assert!(
        load_lap(&root.join("nope.json"))
            .expect("读不到不该报错")
            .is_none()
    );

    // 目录顶替游标：Windows 读目录是 PermissionDenied/Other，Linux 是 IsADirectory，
    // 两者都不是 NotFound ⇒ 必须判红（fail-closed），不许当成"还没开圈"。
    let dir_cursor = root.join("cursor-dir");
    std::fs::create_dir_all(&dir_cursor).expect("建目录型游标");
    assert!(
        load_lap(&dir_cursor).is_err(),
        "目录顶替游标必须判红：当成「还没开圈」就是静默丢证据"
    );

    // 坏 JSON 也要判红（既有那条的形状，保持住）。
    let bad = root.join("bad.json");
    std::fs::write(&bad, "{ not json").expect("写坏游标");
    assert!(load_lap(&bad).is_err(), "游标坏了要判不了");

    let _ = std::fs::remove_dir_all(&root);
}

/// `settle_round` 的 `closing && gaps.is_empty()` 反过来是**假关账**：面全覆盖但键没判齐时
/// 仍然走关账分支，游标清零、已判证据丢一圈，而基线键一直判不齐却没人被点名。
#[test]
fn canary_settle_round_never_closes_a_lap_with_unjudged_baseline_keys() {
    let face = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 5usize)];
    let mut lap = open_lap("main...HEAD", 150, &face, 0);
    // 面已全覆盖（关账条件①满足），但 b.rs 的基线键还没判过。
    lap.rounds = 2;
    lap.covered_files = vec!["a.rs".to_string(), "b.rs".to_string()];
    lap.covered_keys = vec!["a.rs::f".to_string()];
    let baseline = || xtask::mutants::Baseline {
        missed: vec!["a.rs::f".to_string(), "b.rs::g".to_string()],
    };
    let root = std::env::temp_dir().join(format!("adv-settle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("mutants-baseline.json");
    std::fs::write(&path, r#"{"missed":[]}"#).expect("铺基线");

    let mut violations = Vec::new();
    settle_round(
        &mut lap,
        "main...HEAD",
        150,
        &baseline(),
        &path,
        false,
        &mut violations,
    )
    .expect("结算不该报错");

    assert!(
        violations
            .iter()
            .any(|v| v.contains("b.rs::g") || v.contains("没判齐")),
        "键没判齐必须被点名（`&&` 反过来就会静默关账）：{violations:?}"
    );
    assert!(
        !lap.covered_keys.is_empty(),
        "假关账会把游标清零 ⇒ 这条证据就没了：{:?}",
        lap.covered_keys
    );
    assert_eq!(
        lap.completed_laps, 0,
        "没判齐不许计入已关账圈数：{}",
        lap.completed_laps
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 判齐了那一圈才允许关账：证据清零、圈数 +1、面原样带走（`completed_laps + 1` 的
/// `*` / `-` 两个突变在这里露馅——`*` 会把圈数抹成 0）。
#[test]
fn canary_settle_round_closes_when_face_and_keys_are_all_judged() {
    let face = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 5usize)];
    let mut lap = open_lap("main...HEAD", 150, &face, 2);
    lap.rounds = 3;
    lap.covered_files = vec!["a.rs".to_string(), "b.rs".to_string()];
    lap.covered_keys = vec!["a.rs::f".to_string(), "b.rs::g".to_string()];
    lap.missed_keys = vec!["b.rs::g".to_string()];
    let baseline = || xtask::mutants::Baseline {
        missed: vec!["a.rs::f".to_string(), "b.rs::g".to_string()],
    };
    let root = std::env::temp_dir().join(format!("adv-close-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("mutants-baseline.json");
    std::fs::write(&path, r#"{"missed":[]}"#).expect("铺基线");

    let mut violations = Vec::new();
    settle_round(
        &mut lap,
        "main...HEAD",
        150,
        &baseline(),
        &path,
        false,
        &mut violations,
    )
    .expect("关账不该报错");

    assert_eq!(lap.completed_laps, 3, "关账轮要把圈数 +1（不是抹成 0）");
    assert_eq!(lap.rounds, 0, "关账后新一轮从零起");
    assert!(lap.covered_keys.is_empty() && lap.covered_files.is_empty());
    assert_eq!(lap.face, face, "面要原样带进新一轮（重开一圈不等于换面）");
    assert!(
        violations.is_empty(),
        "判齐的一圈不该有违规（--update 未开）：{violations:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// `resume_lap` 的四个突变（`lap.spec == spec` 守卫两个方向、`==` 反过来、丢 budget 字段）
/// 都在"读游标"这条路径上：口径变了却不重开，就等于拿旧证据顶新面。
#[test]
fn canary_resume_lap_reopens_when_the_spec_or_the_face_changed() {
    let face = vec![("a.rs".to_string(), 10usize)];
    let mut stored = open_lap("main...HEAD", 150, &face, 1);
    stored.covered_keys = vec!["a.rs::f".to_string()];
    stored.rounds = 2;

    // ① 同口径、面没漂 ⇒ 证据留着，只更新本轮预算。
    let same = rotation::resume_lap(Some(stored.clone()), "main...HEAD", 99, &face);
    assert_eq!(same.covered_keys, stored.covered_keys, "同口径续用旧证据");
    assert_eq!(same.budget, 99, "本轮预算要覆盖旧游标的");
    assert_eq!(same.completed_laps, 1, "已关账圈数不能被续用丢掉");

    // ② 口径变了 ⇒ 重开一圈（证据清零，已关账圈数保留）。
    let other = rotation::resume_lap(Some(stored.clone()), "abc..HEAD", 150, &face);
    assert!(
        other.covered_keys.is_empty() && other.rounds == 0,
        "口径变了必须重开：{other:?}"
    );
    assert_eq!(other.spec, "abc..HEAD");
    assert_eq!(other.completed_laps, 1, "已关账圈数在重开后保留");

    // ③ 面在圈中漂了 ⇒ 重开（拿旧面顶新面就是移动球门）。
    let drifted = vec![("a.rs".to_string(), 10usize), ("b.rs".to_string(), 1usize)];
    let reopened = rotation::resume_lap(Some(stored.clone()), "main...HEAD", 150, &drifted);
    assert_eq!(reopened.face, drifted, "面漂了就以当前面重新定格");
    assert!(reopened.covered_keys.is_empty(), "面漂了旧证据不顶用");

    // ④ 没有游标 ⇒ 新开一圈，已关账圈数从 0 起。
    let fresh = rotation::resume_lap(None, "main...HEAD", 150, &face);
    assert_eq!(fresh.completed_laps, 0);
    assert_eq!(fresh.spec, "main...HEAD");
    assert_eq!(fresh.budget, 150);
}

/// `close_lap` 里 `--update` 的缺测闸（`lap.resource_rounds > 0`）：写成 `==` 时干净圈
/// 被误拒、写成 `<` 时**有缺测的圈照写基线**（缺测被洗白——这正是 DD-0017 收尾时
/// resource_rounds 判据要挡的事）。两个方向都用真落盘/不落盘钉死。
#[test]
fn canary_close_lap_update_refuses_only_on_resource_shortfalls() {
    let face = vec![("a.rs".to_string(), 10usize)];
    let root = std::env::temp_dir().join(format!("adv-close-upd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("mutants-baseline.json");

    // ① 干净圈（resource_rounds=0）+ --update ⇒ 基线照写，violations 空。
    let mut lap = open_lap("main...HEAD", 150, &face, 0);
    lap.rounds = 1;
    lap.covered_files = vec!["a.rs".to_string()];
    lap.missed_keys = vec!["a.rs::f".to_string()];
    let mut v = Vec::new();
    rotation::close_lap(&mut lap, "main...HEAD", 150, &path, 2, true, &mut v)
        .expect("干净圈关账不该报错");
    assert!(v.is_empty(), "干净圈的 --update 不该被拒：{v:?}");
    let raw = std::fs::read_to_string(&path).expect("基线必须落盘");
    assert!(raw.contains("a.rs::f"), "重录要把 missed 写进去：{raw}");

    // ② 有缺测的圈（resource_rounds=1）+ --update ⇒ 拒绝重录、点名缺测、基线保持原样。
    let mut lap2 = open_lap("main...HEAD", 150, &face, 1);
    lap2.rounds = 1;
    lap2.resource_rounds = 1;
    lap2.covered_files = vec!["a.rs".to_string()];
    lap2.missed_keys = vec!["b.rs::g".to_string()];
    let mut v2 = Vec::new();
    rotation::close_lap(&mut lap2, "main...HEAD", 150, &path, 2, true, &mut v2)
        .expect("缺测圈关账本身不该报错");
    assert!(
        v2.iter().any(|x| x.contains("缺测")),
        "缺测必须点名并拒绝重录（`>` 写成 `<` 时这里会静默洗白）：{v2:?}"
    );
    let raw2 = std::fs::read_to_string(&path).expect("读回基线");
    assert!(
        !raw2.contains("b.rs::g"),
        "拒绝重录 ⇒ 基线保持上一轮内容：{raw2}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
