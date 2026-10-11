//! DD-0018 分诊（adv-rules 格）：**规则注解门的两侧都要有人测**。
//!
//! 诊断实测：63 变异 12 存活。既有 `m1s2/m2s2/ruleid` 里所有 `check_fixture` 调用都是
//! `.unwrap()`——只测"该过能过"，从不测"不该过会拦"，于是 `check_fixture` 整体换成
//! `Ok(())`（门永久静默放行）都没人发现。这里补上失败侧与记账侧。

use adv_parse::{Language, parse_source};
use adv_rules::suppression::collect;
use adv_rules::testing::{annotated_lines, check_fixture};
use adv_rules::{Rule, load_rules};

fn rules() -> Vec<Rule> {
    load_rules(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rules"))
        .expect("真实规则集应当加载成功")
}

fn rs(src: &str) -> adv_parse::GenericAst {
    parse_source(Language::Rust, src).expect("解析 Rust")
}

/// 注解行集合：只收**真的带 ruleid 的**注释行，行号原样。
/// （整体换成 `vec![]`/`vec![0]`/`vec![1]`、以及把 `!extract_ruleids(..).is_empty()` 的
/// `!` 去掉（= 所有注释行都算注解行）——四种都在这一条下暴露。）
#[test]
fn annotated_lines_returns_only_the_ruleid_bearing_lines() {
    let src = "import os\n# ruleid: PY-EVAL-USE\nx = 1\n# 普通注释，不是注解\ny = eval(input())  # ruleid: PY-EVAL-USE\n";
    assert_eq!(
        annotated_lines(src, Language::Python),
        vec![2, 5],
        "只要带 ruleid 的那两行，行号必须照抄 comments 里的 1-based 行号"
    );
    assert!(
        annotated_lines("x = 1\ny = 2\n", Language::Python).is_empty(),
        "没有任何注释 ⇒ 空集（不许凭空给一行）"
    );
}

/// 该过的要过。
#[test]
fn check_fixture_accepts_a_consistent_annotation() {
    // 同一行会命中两条规则（`eval(input())` 既是 eval 使用、也是污点入参），一行可注多条
    let src = "import os\nvalue = eval(input())  # ruleid: PY-EVAL-USE PY-TAINT-EVAL\n";
    check_fixture("py_eval_ok", Language::Python, src, &rules())
        .unwrap_or_else(|e| panic!("注解与实报一致时不该拦，实得：{e}"));
}

/// **不该过的必须拦**：注了规则但那行其实不命中 ⇒ Err，且点名"漏报"并带上规则号。
/// （`check_fixture` 被换成 `Ok(())` 时这条直接红；miss 侧的 `!actual.contains` 取反
/// 会让漏报清单变成空，文案断言同样红。）
#[test]
fn check_fixture_reports_missing_findings_as_underreport() {
    let src = "import os\nsafe = len(input())  # ruleid: PY-EVAL-USE\n";
    let err = check_fixture("py_eval_miss", Language::Python, src, &rules())
        .expect_err("注了规则却没命中必须判红");
    assert!(err.contains("漏报"), "要说清是漏报：{err}");
    assert!(
        err.contains("PY-EVAL-USE"),
        "漏报清单必须逐条带规则号：{err}"
    );
}

/// 反向：真命中但没注 ⇒ Err 且点名"误报"（extra 侧的 `!expected.contains` 取反会在这里红）。
#[test]
fn check_fixture_reports_unannotated_findings_as_overreport() {
    let src = "import os\nvalue = eval(input())\n";
    let err = check_fixture("py_eval_extra", Language::Python, src, &rules())
        .expect_err("命中却未注解必须判红");
    assert!(err.contains("误报"), "要说清是误报：{err}");
    // 断言要锚在**误报那一行**上：只 `err.contains(规则号)` 会被末尾的"实际: …"顺带满足，
    // 于是把 extra 的 `!expected.contains` 取反（误报清单被清空）也照样通过——实测就是这样漏掉的。
    let reported = err
        .lines()
        .find(|l| l.contains("误报（没注却报）"))
        .unwrap_or("");
    assert!(
        reported.contains("PY-EVAL-USE") && reported.contains("PY-TAINT-EVAL"),
        "误报清单必须逐条带上命中的规则号，实得该行：{reported:?}（全文：{err}）"
    );
}

/// 文档注释**不是**注解通道（本模块自己的语法示例就在文档里，否则自指误报）。
/// 四个前缀分支各是一个 `||` 项，`||`→`&&` 会让对应前缀不再被跳过 ⇒ 每个前缀都得单独喂一次。
#[test]
fn doc_comments_are_not_an_annotation_channel() {
    let payload = "adv:allow(RS-UNWRAP-USE, reason=示例, until=2999-01-01)";
    for prefix in ["/// ", "//! "] {
        let src = format!("{prefix}{payload}\nfn main() {{ let v = 1u32; let _ = v.unwrap(); }}\n");
        let (ok, bad) = collect(&rs(&src));
        assert!(
            ok.is_empty() && bad.is_empty(),
            "`{prefix}` 开头的文档注释必须整条跳过，实得 ok={ok:?} bad={bad:?}"
        );
    }
    // ⚠ `/**` 与 `/*!` 两个前缀**当前喂不进来**：rust 的块注释根本不进 `ast.comments`
    // （DD-0019①：采集侧只传单个 COMMENT_KIND）。那两条 `||` 变异要等 ① 修好才可杀。

    // 对照组：同一句话放在**普通**行注释里必须被当成注解——否则上面两条断言是空转。
    let (ok, bad) = collect(&rs(&format!("// {payload}\nfn main() {{}}\n")));
    assert!(bad.is_empty(), "合法抑制不该判畸形：{bad:?}");
    assert_eq!(ok.len(), 1, "普通行注释里的 adv:allow 必须被收进通道");
    assert_eq!(ok[0].rule, "RS-UNWRAP-USE");
    assert_eq!(ok[0].reason, "示例");
    assert_eq!(ok[0].until, "2999-01-01");
}

/// 文件级标记（`adv:allow-file(`）与行级分开解析；先匹配 file 再匹配 line 的顺序也要钉
/// （两个 `find` 的先后被换就会把文件级降成行级）。
#[test]
fn file_scope_marker_is_parsed_as_file_scope() {
    let src = "// adv:allow-file(RS-UNWRAP-USE, reason=整仓放行, until=2999-01-01)\nfn main() {}\n";
    let (ok, bad) = collect(&rs(src));
    assert!(bad.is_empty(), "合法文件级标记不该判畸形：{bad:?}");
    assert_eq!(ok.len(), 1);
    assert_eq!(
        format!("{:?}", ok[0].scope),
        format!("{:?}", adv_rules::suppression::Scope::File),
        "allow-file 必须解析成 File 粒度"
    );
}

/// `load_rules` 只收 `.yaml`/`.yml`：少认一种扩展名会**静默少加载规则**（门看起来在跑，
/// 其实覆盖面小了）。用真实规则文件的内容造三种扩展名，两个必须加载、一个必须被跳过。
#[test]
fn load_rules_accepts_both_yaml_extensions_and_skips_others() {
    // 取一份**真实规则文件**的原文当样本（`Rule` 只有 Deserialize，没法从结构回写）。
    // 自己递归找，不为本测试给包加依赖。
    let rules_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rules");
    let mut stack = vec![rules_root.clone()];
    let mut sample_path = None;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("读规则目录") {
            let p = entry.expect("目录项").path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if ext == "yaml" || ext == "yml" {
                sample_path = Some(p);
                break;
            }
        }
        if sample_path.is_some() {
            break;
        }
    }
    let sample_path =
        sample_path.unwrap_or_else(|| panic!("{} 里应当有规则文件", rules_root.display()));
    let sample = std::fs::read_to_string(&sample_path).expect("读真实规则");
    assert!(
        serde_yaml_ng::from_str::<Rule>(&sample).is_ok(),
        "样本本身必须能当规则解析：{}",
        sample_path.display()
    );

    let dir = std::env::temp_dir().join(format!("adv-rules-ext-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时规则目录");
    for name in ["a.yaml", "b.yml", "notes.txt", "README.md"] {
        std::fs::write(dir.join(name), &sample).expect("写规则样本");
    }

    let got = load_rules(&dir).expect("加载不该失败");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        got.len(),
        2,
        "只该收 .yaml 与 .yml 两份（漏认一种扩展名=静默少加载规则）"
    );
}
