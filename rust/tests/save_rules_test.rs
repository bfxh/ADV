//! S191 存档区两条规则的契约测试（`rust/src/bug/save.rs`）。
//!
//! **金丝雀两头都要有**：违背样本必红（规则真能报）；正确样本必静（原子写/版本门
//! 在场时不报）——只证"能报"会漏掉"见谁都报"的假门，只证"安静"会漏掉死规则。
//! 每条判据都断言行号，不只断言"有命中"（行号错的位置信息是假信息）。
//!
//! 走 `bug::bug_scan`（公开入口）而非内部函数：`bug::save` 是私有子模块，
//! 且经公开入口才连带验证 `scan_one` 的语言分派接线。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rxrs::bug;
use rxrs::json::Value;

/// 进程内单调计数：与 nanos 一起构成目录名，**不靠时钟粒度**保证唯一。
static SEQ: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!(
            "rx-save-test-{}-{}-{}",
            tag,
            n,
            SEQ.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 写一个语料文件并扫描，返回 (rule, line, severity, msg) 四元组（只留 save_* 规则）。
/// `tag` 必须**每个测试各不相同**（兄弟测试文件同惯例：`"core"`/`"rust"`/`"s131"`…）——
/// 首版我给所有测试都用 `"one"`，只靠 nanos 区分；CI 覆盖率档（llvm-cov 插桩、机器时
/// 钟粒度粗）上两个测试落进**同一个临时目录**互相覆盖夹具，于是 `--test save_rules_test`
/// 红。现在 tag 区分 + 进程内计数双保险（见 `temp_dirs_never_collide`）。
fn scan_rules(tag: &str, rel: &str, body: &str) -> Vec<(String, i128, String, String)> {
    let d = TempDir::new(tag);
    let p = d.path().join(rel);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&p, body).unwrap();
    let out = bug::bug_scan(&d.path().to_string_lossy(), 20);
    let mut got = Vec::new();
    if let Some(Value::Arr(items)) = out.get("issues") {
        for it in items {
            let rule = match it.get("rule") {
                Some(Value::Str(s)) => s.clone(),
                _ => continue,
            };
            if !rule.starts_with("save_") {
                continue;
            }
            let line = match it.get("line") {
                Some(Value::Int(i)) => *i,
                _ => -1,
            };
            let sev = match it.get("severity") {
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            };
            let msg = match it.get("msg") {
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            };
            got.push((rule, line, sev, msg));
        }
    }
    got
}

// ---------- 规则一：非原子写 ----------

#[test]
fn nonatomic_write_is_reported_at_exact_line() {
    let got = scan_rules("nonatomic", "main.rs",
        "fn save_game(data: String) {\n    std::fs::write(\"save.ron\", data).unwrap();\n}\n",
    );
    assert_eq!(got.len(), 1, "应恰一处命中：{:?}", got);
    assert_eq!(got[0].0, "save_nonatomic_write");
    assert_eq!(got[0].1, 2, "行号必须指向写点");
    assert_eq!(got[0].2, "med");
    assert!(got[0].3.contains("非原子"), "{}", got[0].3);
}

#[test]
fn atomic_idiom_write_target_is_quiet() {
    // 原子写惯用法：**落盘目标是中间路径**（save_tmp / .tmp 后缀）⇒ 不报（金丝雀"静"的一头）。
    // 这条是本规则的抑制依据——抑制只看写点本身，不看函数里有没有 rename。
    let named = scan_rules("atomic", "main.rs",
        "fn save_game(data: String, save_path: std::path::PathBuf) -> std::io::Result<()> {\n    let save_tmp = save_path.with_extension(\"tmp\");\n    std::fs::write(&save_tmp, data.as_bytes())?;\n    std::fs::sync_all_path(&save_tmp)?;\n    std::fs::rename(&save_tmp, &save_path)\n}\n",
    );
    assert!(named.is_empty(), "中间路径写点不得报：{:?}", named);
}

#[test]
fn direct_write_plus_unrelated_rename_still_reports() {
    // 负面留档（实测否掉的设计）：曾按"函数里出现 rename 就抑制"写，结果
    // "直写存档 + 无关 rename"被误静，且真正的原子写根本走不到这条分支。
    // 现口径：抑制只看写点自身 ⇒ 本夹具必须照报。
    let got = scan_rules("rename", "main.rs",
        "fn save_game(data: String, save_path: std::path::PathBuf) {\n    std::fs::write(&save_path, data.as_bytes()).unwrap();\n    let _ = std::fs::rename(&save_path, archive_path());\n}\n",
    );
    assert_eq!(got.len(), 1, "直写存档不得因无关 rename 被静：{:?}", got);
    assert_eq!(got[0].0, "save_nonatomic_write");
    assert_eq!(got[0].1, 2);
}

#[test]
fn atomic_idiom_in_other_fn_does_not_suppress() {
    // 抑制口径是**所在函数**：同文件别处有原子写，不改变本函数的判定
    let got = scan_rules("otherfn", "main.rs",
        "fn save_game(data: String) {\n    std::fs::write(\"save.ron\", data).unwrap();\n}\n\nfn save_atomic(data: String) {\n    std::fs::write(scratch(), &data).unwrap();\n    std::fs::rename(scratch(), final_path()).unwrap();\n}\n",
    );
    assert_eq!(got.len(), 1, "只应报直写那处：{:?}", got);
    assert_eq!(got[0].1, 2);
}

#[test]
fn temp_target_and_non_save_writes_are_quiet() {
    let temp_target = scan_rules("tempnon", "main.rs",
        "fn flush(save_path: std::path::PathBuf, data: String) {\n    std::fs::write(save_path.with_extension(\"tmp\"), data).unwrap();\n}\n",
    );
    assert!(temp_target.is_empty(), "临时目标不得报：{:?}", temp_target);
    let not_a_save = scan_rules("tempnon", "main.rs",
        "fn log(data: String) {\n    std::fs::write(\"run.log\", data).unwrap();\n}\n",
    );
    assert!(not_a_save.is_empty(), "非存档写不得报：{:?}", not_a_save);
}

#[test]
fn anchors_inside_comment_and_string_are_ignored() {
    let got = scan_rules("comment", "main.rs",
        "// std::fs::write(\"save.ron\", data) 说明见文档\nfn help() -> &'static str {\n    \"把 std::fs::write(\\\"save.ron\\\", x) 换成原子写\"\n}\n",
    );
    assert!(got.is_empty(), "注释/字面量里的锚点不得报：{:?}", got);
}

// ---------- 规则二：读无版本门 ----------

#[test]
fn load_without_version_gate_is_reported() {
    let got = scan_rules("load", "main.rs",
        "fn load_game(text: &str) {\n    let data: SaveData = ron::from_str(text).unwrap();\n    apply(data);\n}\n",
    );
    assert_eq!(got.len(), 1, "应恰一处命中：{:?}", got);
    assert_eq!(got[0].0, "save_load_no_version");
    assert_eq!(got[0].1, 2);
    assert_eq!(got[0].2, "low");
}

#[test]
fn version_gate_in_same_fn_is_quiet() {
    let got = scan_rules("vergate", "main.rs",
        "fn load_game(text: &str) -> Option<SaveData> {\n    let raw = ron::from_str::<RawSave>(text).ok()?;\n    if raw.format_version < CURRENT_SAVE_FORMAT {\n        return migrate(raw);\n    }\n    Some(upgrade(raw))\n}\n",
    );
    assert!(got.is_empty(), "版本门在场不得报：{:?}", got);
}

#[test]
fn non_save_deserialization_is_quiet() {
    let got = scan_rules("nonsave", "main.rs",
        "fn load_defs(text: &str) {\n    let defs: Vec<ModuleDef> = ron::from_str(text).unwrap();\n    register(defs);\n}\n",
    );
    assert!(got.is_empty(), "非存档解析不得报：{:?}", got);
}

#[test]
fn turbofish_and_whitespace_forms_are_matched() {
    let got = scan_rules("turbofish", "main.rs",
        "fn load_game(text: &str) {\n    let d = ron::from_str::<SaveData>(text).unwrap();\n    let e: SaveData = serde_json::from_str (text).unwrap();\n    use_it(d, e);\n}\n",
    );
    assert_eq!(got.len(), 2, "turbofish 与空白两种形态都要认：{:?}", got);
    assert_eq!(got[0].1, 2);
    assert_eq!(got[1].1, 3);
}

// ---------- 降级与语言面 ----------

#[test]
fn temp_dirs_never_collide() {
    // 同一 tag 连续取两个临时目录必须不同——首版只靠 nanos 区分，CI 覆盖率档（插桩 +
    // 机器时钟粒度粗）上两个测试落进同一目录、互相覆盖夹具 ⇒ `--test save_rules_test` 红。
    // 本判据把"不靠时钟粒度"钉住（tag 区分 + 进程内 SEQ 兜底）。
    let a = TempDir::new("collide");
    let b = TempDir::new("collide");
    assert_ne!(a.path(), b.path(), "同 tag 也必须拿到不同目录");
}

#[test]
fn test_region_downgrades_message_and_severity() {
    let got = scan_rules("downgrade", "main.rs",
        "fn save_game(data: String) {\n    std::fs::write(\"save.ron\", data).unwrap();\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn roundtrip() {\n        std::fs::write(\"save.ron\", \"x\").unwrap();\n        let d: SaveData = ron::from_str(\"x\").unwrap();\n    }\n}\n",
    );
    assert!(!got.is_empty(), "测试区仍要报（只是降级）");
    for (_, line, sev, msg) in &got {
        if *line > 4 {
            assert_eq!(sev, "low", "测试区降为 low：{:?}", got);
            assert!(msg.contains("降级"), "测试区要带降级标注：{}", msg);
        }
    }
    assert_eq!(got.len(), 3, "生产 1 + 测试 2：{:?}", got);
}

#[test]
fn tests_segment_path_downgrades_without_cfg_attr() {
    let got = scan_rules("segpath", "tests/save_flow.rs",
        "fn writes_save() {\n    std::fs::write(\"save.ron\", \"x\").unwrap();\n}\n",
    );
    assert_eq!(got.len(), 1, "{:?}", got);
    assert_eq!(got[0].2, "low");
    assert!(got[0].3.contains("降级"), "{}", got[0].3);
}

#[test]
fn python_surface_is_not_scanned_by_save_rules() {
    // 规则是 Rust 语言面：同名文本落在 .py 里不归这两条（各自语言面另有规则）
    let got = scan_rules("pysurf", "tool.py",
        "def save():\n    fs.write(\"save.ron\", data)\n",
    );
    assert!(got.is_empty(), "Rust 语言面的规则不得在 .py 上开火：{:?}", got);
}

#[test]
fn extended_name_is_not_a_call_anchor() {
    // 左词边界（实测缺陷：测量探针首跑列出的 `extend_from_slice` 一类）——
    // 名字被别的标识符"吞"进去时不算锚点，哪怕整行含存档 token。
    let got = scan_rules("extname", "main.rs",
        "fn pack(save_buf: &mut Vec<u8>, chunk: &[u8]) {\n    save_buf.extend_from_slice(chunk);\n}\n",
    );
    assert!(got.is_empty(), "extend_from_slice 不是 from_slice 调用：{:?}", got);
}

#[test]
fn scan_is_deterministic() {
    let body = "fn save_game(d: String) {\n    std::fs::write(\"save.ron\", d).unwrap();\n}\nfn load(t: &str) {\n    let x: SaveData = ron::from_str(t).unwrap();\n}\n";
    let first = scan_rules("determ", "main.rs", body);
    let second = scan_rules("determ", "main.rs", body);
    assert_eq!(first.len(), 2, "{:?}", first);
    assert_eq!(format!("{:?}", first), format!("{:?}", second), "两跑必须逐字相同");
}
