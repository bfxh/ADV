//! `adv snapshot` 的**参数分支**契约（不碰网络）：变异门实测这一片 7 条变异全存活——
//! 因为只有"真跑一次同步"才会走到它，而真跑要网络。这里把不需要网络的那几条分支钉住：
//! 未知参数、缺目录、多目录都必须是用法错（exit 2），且**不许**悄悄退 0。

use std::process::Command;

fn adv(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .args(args)
        .output()
        .expect("adv 跑不起来");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn no_directory_is_a_usage_error() {
    let (code, _out, stderr) = adv(&["snapshot"]);
    assert_eq!(code, 2, "缺目录要判用法错：{stderr}");
    assert!(stderr.contains("用法"), "{stderr}");
}

#[test]
fn two_directories_is_a_usage_error() {
    let (code, _out, stderr) = adv(&["snapshot", "one", "two"]);
    assert_eq!(code, 2, "只收一个目录：{stderr}");
}

#[test]
fn an_unknown_flag_is_refused_by_name() {
    let (code, _out, stderr) = adv(&["snapshot", "--wat", "dir"]);
    assert_eq!(code, 2, "不认得的参数要判 2 并点名：{stderr}");
    assert!(stderr.contains("--wat"), "报错要点名那个参数：{stderr}");
}
// 刻意**不测** `--with-rustsec` 的 happy path：它会真去 `git clone` 上游库（实测一条用例跑了
// 345 秒、还在仓里建了个目录）。那条路径的背书是账本里那次真桶同步（`adv snapshot` 实跑），
// 以及 `sync_rustsec`/`git` 这几个**起外部进程的薄壳**在变异债里的登记——不在这里假装测到。
