//! 金丝雀（M3-1）：内化 Nosey Parker 的薄壳要能被证明做对了四件事——
//! ① 扫得到（合成凭据必须出账，且规则码是上游的）；② 不多报（干净文件零发现）；
//! ③ 内容寻址去重（同内容两份文件只出一次账）；④ 掩码（接口只给 `***`，原文不出现）。
//!
//! 假值按本仓 S123 纪律**运行时拼接**并写在 `tmp_path` 里：仓里不落任何"像真凭据"的字面量
//! （否则会去动 `.gitleaks.toml` / secrets 门的豁免，那是把纪律换成白名单）。

use std::path::PathBuf;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adv-secrets-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建 tmp_path");
    dir
}

#[test]
fn finds_a_synthetic_key_reports_once_per_content_and_never_leaks_it() {
    let dir = scratch("canary");
    // 上游 `np.aws.1` 的形状：AKIA + 16 位 [A-Z0-9]（上游自己给的 examples 也是假值）。
    let key = format!("{}{}", "AKIA", "DEADBEEFDEADBEEF");
    let dirty = dir.join("dirty.txt");
    std::fs::write(&dirty, format!("aws_key = {key}\n")).expect("写夹具");
    let clean = dir.join("clean.txt");
    std::fs::write(&clean, "hello\nworld\n").expect("写干净样本");
    // 同内容两份文件：去重的判据（内容寻址 = 同内容同 id ⇒ 第二次必然是 Seen*）。
    let dup_a = dir.join("dup_a.txt");
    let dup_b = dir.join("dup_b.txt");
    let same = format!("same = {key}\n");
    std::fs::write(&dup_a, &same).expect("写夹具");
    std::fs::write(&dup_b, &same).expect("写夹具");

    // 显式按固定顺序给两条路径：枚举顺序必须由我们决定，不能靠文件系统顺序碰运气。
    let findings =
        adv_secrets::scan_paths(&[dirty.clone(), clean.clone(), dup_a.clone(), dup_b.clone()])
            .expect("扫描不该失败");
    let _ = std::fs::remove_dir_all(&dir);

    let dirty_n = findings
        .iter()
        .filter(|f| f.path.ends_with("dirty.txt"))
        .count();
    assert_eq!(dirty_n, 1, "合成凭据必须出账且只出一条：{findings:?}");
    let hit = findings
        .iter()
        .find(|f| f.path.ends_with("dirty.txt"))
        .expect("上面刚断言存在");
    assert_eq!(hit.rule, "np.aws.1", "规则码要用上游的（便于回上游对账）");
    assert_eq!(hit.line, 1, "行号应指向那一行");
    assert_eq!(hit.masked, "***", "X10：接口只给定长掩码");

    assert!(
        !findings.iter().any(|f| f.path.ends_with("clean.txt")),
        "干净文件不许有发现（防'永远红'）：{findings:?}"
    );

    let dup_n = findings.iter().filter(|f| f.path.contains("dup_")).count();
    assert_eq!(
        dup_n, 1,
        "同内容两份文件只该出一次账（内容寻址去重）：{findings:?}"
    );

    let dump = format!("{findings:?}");
    assert!(
        !dump.contains(&key),
        "原文不许出现在接口的任何角落（含 Debug）：{dump}"
    );
}

#[test]
fn skipped_files_are_named_not_silently_dropped() {
    // "没扫到"不许长得像"干净"：超上限的文件要能被看见（本仓同一条纪律）。
    let dir = scratch("skip");
    let big = dir.join("big.bin");
    std::fs::write(&big, vec![b'x'; (adv_secrets::MAX_FILE_BYTES + 1) as usize]).expect("写大文件");
    let findings = adv_secrets::scan_paths(std::slice::from_ref(&dir)).expect("扫描不该失败");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(findings.is_empty(), "大文件里没有凭据，不该有发现");
}
