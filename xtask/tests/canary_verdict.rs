//! 金丝雀（门必须留痕 `xtask::verdict`）：绿 / 红 / 判不了 三态，且没有第四种"没说话"。

use xtask::verdict::verdict;

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
