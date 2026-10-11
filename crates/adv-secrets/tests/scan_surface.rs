//! DD-0018 分诊（adv-secrets 格）：把 `scan_paths` / `enumerate_files` / `enumerate_skip` /
//! `scan_one` 四个键的存活变异逐个钉死。四件事此前没有面能看见：
//! ① 行号是 `1 + 匹配偏移前的换行数`（金丝雀只测过第 1 行 ⇒ `+`→`-` 存活，因为 `1-0` 还是 1）；
//! ② 目录跳过名只作用于**目录**（同名文件必须照常扫 ⇒ `&&`→`||`、`delete !` 存活）；
//! ③ 枚举必须真的下钻目录（`enumerate_files → Ok(vec![])`、`enumerate_skip → true/false` 全存活）；
//! ④ 大小上限的边界值（恰好 `MAX_FILE_BYTES` 要扫、超一个字节要跳 ⇒ `>` 的三种替换与常数的四个 `*` 全存活）。
//!
//! 假值按 S123 纪律运行时拼接，只落在 `tmp_path`，仓里不留"像真凭据"的字面量。

use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adv-sec-surface-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建 tmp_path");
    dir
}

/// 拼一个合成 AWS access key：`AKIA` + 16 位 `[A-Z0-9]`（上游 `np.aws.1` 的形状）。
/// tag 用来保证每份夹具内容互不相同——同内容会在同一次调用里被内容寻址去重，那就测不到枚举了。
fn fake_key(tag: &str) -> String {
    let mut suffix = String::from("CAFE");
    suffix.extend(
        tag.bytes()
            .map(|b| (b as char).to_ascii_uppercase())
            .filter(|c| c.is_ascii_alphanumeric()),
    );
    suffix.truncate(16);
    while suffix.len() < 16 {
        suffix.push((b'A' + (suffix.len() as u8 % 26)) as char);
    }
    format!("AKIA{suffix}")
}

/// 恰好 `total` 字节、第 1 行带一个键的内容；填充用同一字符（零熵，不该命中任何规则）。
fn one_line_padded(total: usize, tag: &str) -> String {
    let prefix = format!("k = {}\n", fake_key(tag));
    assert!(prefix.len() <= total, "夹具太小放不下表头");
    format!("{}{}", prefix, "p".repeat(total - prefix.len()))
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("建父目录");
    }
    std::fs::write(path, content).expect("写夹具");
}

/// ①②③：目录遍历、跨行行号定位、以及"跳过名只作用于目录"。
#[test]
fn walking_a_tree_reports_lines_and_skips_only_directories() {
    let dir = scratch("walk");
    // 行号：第 2 行和第 5 行各一个键（第 1 行已由金丝雀覆盖）。
    write(
        &dir.join("lines.txt"),
        &format!(
            "clean line\nk = {}\nfiller\nmore\nk = {}\n",
            fake_key("l2"),
            fake_key("l5")
        ),
    );
    // 嵌套目录里的第 4 行：`enumerate_files → Ok(vec![])`、`delete !` 在这条上现形。
    write(
        &dir.join("deep/nested/key.txt"),
        &format!("a\nb\nc\nk = {}\n", fake_key("deep")),
    );
    // 四个跳过名：作为**目录**必须被跳过；同名**文件**必须照常扫。
    for name in ["target", ".git", "node_modules", "__pycache__"] {
        write(
            &dir.join(name).join("planted.txt"),
            &format!("k = {}\n", fake_key(&format!("{name}dir"))),
        );
        write(
            &dir.join("keep").join(name),
            &format!("k = {}\n", fake_key(&format!("{name}file"))),
        );
    }

    let findings = adv_secrets::scan_paths(std::slice::from_ref(&dir))
        .unwrap_or_else(|e| panic!("扫描不该失败（枚举返回空路径会走这条）：{e}"));
    let _ = std::fs::remove_dir_all(&dir);

    let aws_lines = |suffix: &str| -> Vec<usize> {
        let mut v: Vec<usize> = findings
            .iter()
            .filter(|f| f.path.ends_with(suffix) && f.rule == "np.aws.1")
            .map(|f| f.line)
            .collect();
        v.sort_unstable();
        v
    };

    assert_eq!(
        aws_lines("lines.txt"),
        vec![2, 5],
        "行号 = 1 + 匹配偏移前的换行数：`+`→`-` 会把 5 变成 4（第 1 行时 1-0 仍是 1，所以必须有非首行的命中），`+`→`*` 与 `==`→`!=` 会把它放大：{findings:?}"
    );
    assert_eq!(
        aws_lines("deep/nested/key.txt"),
        vec![4],
        "嵌套目录里的第 4 行要能被定位（这条同时钉住枚举真的下了钻）：{findings:?}"
    );

    // 跳过名当文件用时不许被跳过（`&&`→`||` 会把它连带跳掉）。
    for name in ["target", ".git", "node_modules", "__pycache__"] {
        let suffix = format!("keep/{name}");
        assert!(
            findings.iter().any(|f| f.path.ends_with(&suffix)),
            "名为 {suffix} 的**文件**该照常扫（跳过只作用于目录）：{findings:?}"
        );
    }
    // 跳过名当目录用时里面的文件不许出账（`enumerate_skip → false` 会让这些都冒出来）。
    for name in ["target", ".git", "node_modules", "__pycache__"] {
        let leak = format!("/{name}/planted.txt");
        assert!(
            !findings.iter().any(|f| f.path.ends_with(&leak)),
            "{name} 目录下的文件必须被跳过：{findings:?}"
        );
    }
    for f in &findings {
        assert_eq!(f.masked, "***", "X10：接口只给定长掩码");
        assert!(
            !format!("{f:?}").contains(&fake_key("l5")),
            "原文不许出现在接口的任何角落：{f:?}"
        );
    }
}

/// ④：大小上限的两侧——恰好等于上限要扫（`>`→`>=`/`==` 会把它跳掉），超一个字节要跳；
/// 并把 `MAX_FILE_BYTES` 的四个 `*` 替换钉死（改成加/除会把上限变成 9216、1048584、8、0 量级）。
#[test]
fn the_size_ceiling_is_exclusive_and_its_arithmetic_is_pinned() {
    let dir = scratch("size");
    let max = adv_secrets::MAX_FILE_BYTES as usize;
    assert_eq!(
        max,
        8 * 1024 * 1024,
        "上限应当是 8 MiB：常数里的 `*` 若被换成 `+`/`-`/`/` 先在这条上红"
    );
    write(&dir.join("s20k.txt"), &one_line_padded(20_000, "a"));
    write(&dir.join("s2m.txt"), &one_line_padded(2 * 1024 * 1024, "b"));
    write(&dir.join("exact.txt"), &one_line_padded(max, "c"));
    write(&dir.join("over.txt"), &one_line_padded(max + 1, "d"));

    let findings = adv_secrets::scan_paths(std::slice::from_ref(&dir)).expect("扫描不该失败");
    let _ = std::fs::remove_dir_all(&dir);

    let hit = |suffix: &str| findings.iter().any(|f| f.path.ends_with(suffix));
    assert!(hit("s20k.txt"), "20 KiB 远小于上限，必须扫到：{findings:?}");
    assert!(
        hit("s2m.txt"),
        "2 MiB 小于 8 MiB 上限，必须扫到：{findings:?}"
    );
    assert!(
        hit("exact.txt"),
        "恰好等于上限不该被跳（判据是 >，不是 >=/==）：{findings:?}"
    );
    assert!(
        !hit("over.txt"),
        "超上限一个字节必须被跳（skip_notice 留痕，但接口不许出账）：{findings:?}"
    );
}

/// ④′：跳过留痕的纯函数真值表。`skip_notice` 的意义就是"没扫到不许长得像干净"，
/// 而 `eprintln!` 在测试里不可观察，所以把判据做成可断言的返回值。
#[test]
fn skip_notice_exists_exactly_when_something_was_skipped() {
    assert!(
        adv_secrets::skip_notice(&[]).is_none(),
        "没跳过就不该产出任何留痕文本"
    );
    let one = adv_secrets::skip_notice(&["a/b.bin（9 字节，超上限）".to_string()])
        .expect("非空就该有留痕");
    assert!(
        one.contains("跳过 1 个文件") && one.contains("a/b.bin（9 字节，超上限）"),
        "留痕要带数量和被跳过的文件名，实得：{one:?}"
    );
    let two =
        adv_secrets::skip_notice(&["one".to_string(), "two".to_string()]).expect("非空就该有留痕");
    assert!(
        two.contains("跳过 2 个文件") && two.contains("\n  - one\n  - two"),
        "多条要逐条列出来，实得：{two:?}"
    );
    assert_eq!(
        adv_secrets::skip_notice(&["".to_string()]).as_deref(),
        Some("adv-secrets：跳过 1 个文件：\n  - "),
        "留痕文案逐字节钉住（`skip_notice → None` / `→ Some(\"\")` / `→ Some(\"xyzzy\")` 都在这条上现形）"
    );
}
