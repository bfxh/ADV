//! god 门自己的金丝雀（单列成文件）：`crates/` 里有个真 crate、但没进 `MEMBER_DIRS`
//! ⇒ 计量必须报错而不是静默跳过。
//!
//! 为什么要单列：它原先是塞进 `canary.rs` 末尾的，结果**把 python god 的大括号启发式解析带偏**
//! （一个 fn 一路吞到文件尾，报出 456 行 > 硬阈 120）——门的解析器也是要维护的代码，
//! 不该用测试文件的写法去跟它玩命。

use xtask::god::{MEMBER_DIRS, collect_workspace_entries};

/// 反方向对账（2026-10-16 合并 uiscan 时实测出来的洞）：`crates/` 里有个真 crate、
/// 但没进 `MEMBER_DIRS` ⇒ 计量必须**报错**而不是静默跳过。
/// 当天的症状：rust god 报"新增 0 项"（它压根没扫新 crate），python god 照出 16 个新文件。
#[test]
fn god_bails_when_a_crate_dir_is_not_registered() {
    let root = std::env::temp_dir().join(format!("adv-god-unregistered-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in MEMBER_DIRS {
        std::fs::create_dir_all(root.join(dir)).expect("建成员目录");
    }
    let stray = root.join("crates/uiscan-stray");
    std::fs::create_dir_all(stray.join("src")).expect("建未登记 crate");
    std::fs::write(stray.join("Cargo.toml"), "[package]\nname=\"x\"\n").expect("写清单");
    std::fs::write(stray.join("src/lib.rs"), "pub fn x() {}\n").expect("写源码");

    let err = collect_workspace_entries(&root).expect_err("未登记的 crate 必须判红");
    let _ = std::fs::remove_dir_all(&root);
    let msg = format!("{err}");
    assert!(
        msg.contains("uiscan-stray") && msg.contains("MEMBER_DIRS"),
        "报错要点名：{msg}"
    );
}
