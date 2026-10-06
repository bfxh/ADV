//! 金丝雀（门必须留痕 `xtask::verdict`）：绿 / 红 / 判不了 三态，且没有第四种"没说话"。

use xtask::verdict::{merge_gate, verdict};

#[test]
fn canary_verdict_has_three_states_and_none_of_them_is_silence() {
    // DD-0009 的核心：门只有"绿/红/判不了"三种终点，**没有第四种"没说话"**。
    assert_eq!(
        verdict("mutants", &Ok(vec![])),
        ("mutants: 绿".to_string(), 0),
        "金丝雀失败：绿态文案或退出码漂了"
    );
    let (line, code) = verdict("mutants", &Ok(vec!["a".into(), "b".into()]));
    assert_eq!(
        (line.as_str(), code),
        ("mutants: 红（2 条）\n  - a\n  - b", 1)
    );
    let (line, code) = verdict("mutants", &Err(anyhow::anyhow!("cargo mutants 没跑完")));
    assert_eq!(code, 3, "金丝雀失败：判不了必须与红用不同退出码");
    assert!(line.starts_with("mutants: 判不了"), "{line}");
    assert!(line.contains("本轮没出判定"), "{line}");
    // 多行原因要压成一行：裁决行被拆成多行就没法"看最后一行"。
    let (line, _) = verdict("gate", &Err(anyhow::anyhow!("第一行\n第二行")));
    assert_eq!(line.matches('\n').count(), 0, "{line}");
    assert!(line.contains("第一行 | 第二行"), "{line}");
}

#[test]
fn canary_gate_binary_always_ends_with_a_verdict_line() {
    // 真路径自检：起 `xtask` 二进制，三种终点都必须以 `<门>:` 行收尾。
    // 只测内部函数不够——2026-10-06 那两轮"门哑"就是 main 的错误分支绕过了裁决。
    let xtask = env!("CARGO_BIN_EXE_xtask");

    let mut cmd = std::process::Command::new(xtask);
    cmd.args(["mutants", "--base", "no-such-ref-xyzzy-die"]);
    let out = cmd.output().expect("起 xtask 失败");
    assert_eq!(
        out.status.code(),
        Some(3),
        "取不到 diff 该判「判不了」（不是红、也不是绿）"
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("mutants: 判不了"), "{text}");
    assert!(text.contains("--base"), "要点名是哪个参数坏了：{text}");

    // 空 diff 那一步**不能照搬本地断言**：变异门会把树复制到没有 `.git` 的临时目录，
    // 那里 `git diff HEAD...HEAD` 本身就取不到 ⇒ 门给出"判不了"而不是"红（skip）"。
    // 2026-10-06 实测：我原来写死 `exit == 1`，于是 cargo-mutants 的**未变异基线**直接红
    // （exit 4，"判不了"那行把这件事说清了）。两种环境下都必须成立的只有不变式本身：
    // **一定有一行裁决**，且退出码落在三态里。
    let out = std::process::Command::new(xtask)
        .args(["mutants", "--base", "HEAD"])
        .output()
        .expect("起 xtask 失败");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let in_git_repo = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .is_ok_and(|o| o.status.success());
    if in_git_repo {
        // 在册仓里：无差异 ⇒ 红（skip 不算绿），且这轮没跑 cargo-mutants，所以很快。
        assert_eq!(out.status.code(), Some(1), "skip 必须判红：{text}");
        assert!(text.contains("mutants: 红（1 条）"), "{text}");
        assert!(text.contains("skip 不算绿"), "{text}");
    } else {
        assert!(
            text.contains("mutants: 判不了"),
            "没有 .git 的复制树里也该有一行裁决：{text}"
        );
        assert_eq!(out.status.code(), Some(3), "{text}");
    }
    assert!(
        text.contains("mutants: "),
        "无论哪种环境，终点都必须是一行裁决：{text}"
    );

    // 未知子命令 = 用法错，退出码与三种裁决都可区分。
    let out = std::process::Command::new(xtask)
        .arg("not-a-gate")
        .output()
        .expect("起 xtask 失败");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn canary_verdict_line_goes_to_the_stream_the_code_implies() {
    // 门报 `main.rs::emit` 的 `== → !=` 存活：只读合并流（stdout+stderr）分不出裁决写到了哪一路，
    // 而"绿走 stdout、红/判不了走 stderr"是契约的一部分——重放脚本要能只看 stdout 就取到绿证。
    let xtask = env!("CARGO_BIN_EXE_xtask");
    let out = std::process::Command::new(xtask)
        .args(["mutants", "--base", "no-such-ref-xyzzy-die"])
        .output()
        .expect("起 xtask 失败");
    assert_eq!(out.status.code(), Some(3));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("mutants: 判不了"),
        "判不了必须走 stderr：{stderr}"
    );
    assert!(
        !stdout.contains("mutants:"),
        "非绿态的裁决不许出现在 stdout 上（否则只看 stdout 的重放会把红读成没判）：{stdout}"
    );

    // 空 diff 那一路同理：在册仓里它是红（skip 不算绿），也必须在 stderr。
    let out = std::process::Command::new(xtask)
        .args(["mutants", "--base", "HEAD"])
        .output()
        .expect("起 xtask 失败");
    if out.status.code() == Some(1) {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(stderr.contains("mutants: 红（1 条）"), "{stderr}");
        assert!(!stdout.contains("mutants:"), "红走错了流：{stdout}");
    }
}

/// `gate` 档的裁决必须与单独跑各子门的结果一致。
/// 由来：门报 `gate_run` 三条存活（`Ok(vec![])` / `vec![String::new()]` / `vec!["xyzzy"]`）——
/// 我把三个子步合进一个函数之后，没有任何测试观察过这个聚合点。
#[test]
fn canary_gate_verdict_matches_its_own_subgates() {
    let xtask = env!("CARGO_BIN_EXE_xtask");
    let run = |args: &[&str]| {
        let o = std::process::Command::new(xtask)
            .args(args)
            .output()
            .expect("起 xtask 失败");
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (god_code, _, god_err) = run(&["god"]);
    let (sup_code, _, sup_err) = run(&["suppress"]);
    let sub_red = god_code != Some(0) || sup_code != Some(0);
    let sub_bullets = god_err
        .lines()
        .chain(sup_err.lines())
        .filter(|l| l.starts_with("  - "))
        .count();

    let (gate_code, gate_out, gate_err) = run(&["gate"]);
    if sub_red {
        assert_eq!(gate_code, Some(1), "子门有红而 gate 不红：{gate_out}");
        let gate_bullets = gate_err.lines().filter(|l| l.starts_with("  - ")).count();
        assert!(
            gate_bullets >= sub_bullets,
            "gate 的明细比子门单独跑还少（聚合点被掏空？）：{gate_err}"
        );
    } else {
        // 干净树上：聚合点被改成 `vec![String::new()]` / `vec!["xyzzy"]` 会说"红（1 条）"，
        // 而 gate_run → `Ok(vec![])` 在这一刻与真相同值——那一条属环境等价变异，如实记账。
        assert_eq!(gate_code, Some(0), "子门全绿而 gate 不绿：{gate_err}");
        assert!(gate_out.contains("gate: 绿"), "{gate_out}");
        assert!(!gate_err.contains("  - "), "绿档不该带任何明细：{gate_err}");
    }
}

#[test]
fn canary_merge_gate_keeps_every_subgate_item_and_its_prefix() {
    // 聚合逻辑抽成纯函数才能钉住：`gate_run` 整体换成空清单在绿树上与真值同值（残留那条
    // 已如实登记为 DD-0011），但"谁明细被吞掉"这件事在这里是可观察的。
    let empty: Vec<String> = vec![];
    assert_eq!(
        merge_gate(&[("god", empty.clone()), ("suppress", empty.clone())]),
        empty
    );
    let got = merge_gate(&[
        ("god", vec!["棘轮 a = 2 > 基线 1".to_string()]),
        (
            "suppress",
            vec!["抑制到期 b".to_string(), "畸形 c".to_string()],
        ),
    ]);
    assert_eq!(got.len(), 3, "子门明细被吞了：{got:?}");
    assert_eq!(
        got,
        vec![
            "god: 棘轮 a = 2 > 基线 1".to_string(),
            "suppress: 抑制到期 b".to_string(),
            "suppress: 畸形 c".to_string(),
        ],
        "聚合必须保留顺序、条数与子门名前缀"
    );
}
