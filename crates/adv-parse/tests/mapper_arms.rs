//! DD-0018 分诊（adv-parse 格）：语言映射器的**分支形状**覆盖。
//!
//! 既有 `ast_snap.rs` 是 insta 快照，夹具只喂了 `import os` / `use std::env` 这一类最简形，
//! 于是 `import_kind` / `use_tree_paths` / `dotted_callee` 的多数 match 臂、以及
//! `collect_comments` / `note_error` / `GenericAst::string` 的记账面**一条都没被观察到**
//! （诊断实测：97 变异 32 存活）。这里按"每个臂至少一个真形状"补断言，全部走
//! `parse_source` 真解析（不 mock、不注入）。
//!
//! ⚠ 补断言过程中坐实了**三条映射器语义缺陷**，登记为 **DD-0019**（本批判是清覆盖，
//! 不动归一 AST 的语义）。这三处下面按**当前行为**钉死并标了号——不是给错行为背书：
//! 修 DD-0019 时这些断言必须连夹具一起改（改对了它们就会红，这是有意的绊线）。

use adv_parse::{AstKind, GenericAst, Language, ParseError, parse_source};

/// 收集 `Import` 节点的 (module, names)。
fn imports(ast: &GenericAst) -> Vec<(String, Vec<(String, String)>)> {
    ast.nodes
        .iter()
        .filter_map(|n| match &n.kind {
            AstKind::Import { module, names } => Some((module.clone(), names.clone())),
            _ => None,
        })
        .collect()
}

/// 收集调用名（`dotted_callee` 的落点）。
fn callees(ast: &GenericAst) -> Vec<String> {
    ast.nodes
        .iter()
        .filter_map(|n| match &n.kind {
            AstKind::Call { callee, .. } => Some(callee.clone()),
            _ => None,
        })
        .collect()
}

fn all_import_names(ast: &GenericAst) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = imports(ast).into_iter().flat_map(|(_, n)| n).collect();
    v.sort();
    v
}

fn has_names(ast: &GenericAst, want: &[(&str, &str)]) -> bool {
    let got = all_import_names(ast);
    want.iter()
        .all(|(a, b)| got.contains(&(a.to_string(), b.to_string())))
}

/// `GenericAst::string` 必须原样取回驻留文本（换成空串或常量在真实映射里无人发现）。
#[test]
fn string_returns_the_interned_text_for_every_name_field() {
    let ast = parse_source(
        Language::Python,
        "def h(p):\n    pass\n\nclass Widget:\n    pass\n",
    )
    .expect("解析 Python");

    let names: Vec<String> = ast
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            AstKind::FunctionDef { name } => Some(ast.string(*name).to_string()),
            AstKind::ClassDef { name } => Some(ast.string(*name).to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["h".to_string(), "Widget".to_string()]);
}

/// 错误类型的 Display 必须带上原文——`parse_source` 的失败路径靠这句话定位。
#[test]
fn parse_error_display_carries_the_inner_message() {
    let e = ParseError::Internal("boom".to_string());
    let s = e.to_string();
    assert!(
        s.contains("解析器内部错误") && s.contains("boom"),
        "Display 要既是人话又带上原文，实得：{s:?}"
    );
}

/// 注释记账：内容 + **1-based 行号**。`row + 1` 写成 `row - 1` / `row * 1` 只有在这里才露馅，
/// 而行号是规则测试门禁（`ruleid:` 注释）挂规则用的，错一行就把标记挂到别的行上。
#[test]
fn comments_are_recorded_with_one_based_lines() {
    let src = "# 首行\nx = 1\n# 第三行\ny = 2\n";
    let ast = parse_source(Language::Python, src).expect("解析带注释的 Python");
    assert_eq!(
        ast.comments,
        vec![
            (1usize, "# 首行".to_string()),
            (3usize, "# 第三行".to_string())
        ],
        "注释必须按源码顺序、行号 1-based"
    );

    // Rust 的**行注释**同判据（两种语言共用同一个 collect_comments，只测一边等于没测另一边）。
    let rs = "// r 一行\nfn main() {}\n\n/* 块四行 */\n";
    let ast = parse_source(Language::Rust, rs).expect("解析带注释的 Rust");
    assert_eq!(
        ast.comments,
        vec![(1usize, "// r 一行".to_string())],
        "rust 行注释要按 1-based 记账"
    );
    // ⚠ **DD-0019①**：第 4 行的块注释当前**不记账**（`mapper.rs` 的采集侧每语言只传单个
    // `COMMENT_KIND`，rust 是 `line_comment`；而消费侧 `rust_lang.rs` 一直按 `COMMENT_KINDS`
    // 两种排除）。后果：写在 `/* */` 里的 `ruleid:` 标记对规则门禁隐形。
    // 这条断言是**有意钉住缺陷**：修 DD-0019① 时它会红，届时把期望改成 [(1,..),(4,..)]。
    assert!(
        !ast.comments.iter().any(|(_, t)| t.contains("块四行")),
        "DD-0019① 已修复的话删除本断言，并把上面那条改成含第 4 行"
    );
}

/// 残缺语法照常产 AST 但必须计数；干净源码必须计 0。两个方向都钉，才照得出
/// `is_error() || is_missing()` 写成 `&&`、以及 `+= 1` 写成 `-= 1` / `*= 1`。
#[test]
fn parse_error_counter_counts_broken_and_ignores_clean() {
    let clean = parse_source(Language::Python, "def f():\n    return 1\n").expect("干净源码");
    assert_eq!(clean.parse_errors, 0, "干净源码不该计出残缺节点");

    let broken = parse_source(Language::Python, "def f(:\n    ????\n").expect("残缺也照常产出");
    assert!(
        broken.parse_errors > 0,
        "残缺语法必须计数（报 0 = 这条自检失去分辨力，下游会以为源码是完整的）"
    );
}

/// `dotted_callee`（Python）：属性调用与裸名调用两个臂。
#[test]
fn python_dotted_callee_covers_bare_and_attribute_targets() {
    let ast = parse_source(
        Language::Python,
        "import os\ndef t():\n    os.system('ls')\n    helper()\n",
    )
    .expect("解析 Python 调用");
    let got = callees(&ast);
    assert!(
        got.contains(&"os.system".to_string()) && got.contains(&"helper".to_string()),
        "属性调用要还原成点分名、裸名要原样，实得：{got:?}"
    );
}

/// `import_kind`（Python）：别名 / 通配 / 裸多段 / from 四种形状各自的落点。
#[test]
fn python_import_kind_covers_alias_wildcard_dotted_and_from() {
    // `import os as o` ⇒ aliased_import 臂：(原名, 别名)
    let ast = parse_source(Language::Python, "import os as o\n").expect("别名导入");
    assert_eq!(
        all_import_names(&ast),
        vec![("os".to_string(), "o".to_string())],
        "aliased_import 必须展开成 (原名, 别名)"
    );

    // `import a.b`（无别名）⇒ 绑定根段 a（不是恒等映射；防把 a.b 拼成 a.b.b 的那条理由）
    let ast = parse_source(Language::Python, "import a.b\n").expect("多段导入");
    assert_eq!(
        all_import_names(&ast),
        vec![("a.b".to_string(), "a".to_string())],
        "import a.b 要绑根段 a，且不许被 from 分支改写成恒等"
    );

    // `from os import *` ⇒ wildcard_import 臂必须产出 (*, *)
    let ast = parse_source(Language::Python, "from os import *\n").expect("通配导入");
    assert!(
        has_names(&ast, &[("*", "*")]),
        "wildcard_import 臂要把通配记成 (*, *)，实得：{:?}",
        all_import_names(&ast)
    );
    assert_eq!(imports(&ast)[0].0, "os", "from 的模块名必须落在 module 上");
    // ⚠ **DD-0019②**：模块名同时也被当成了绑定名 ⇒ names 里多出 ("os","os")。
    // `from os import X` 并不绑定 `os`，这条污染下游的导入映射。修好时删掉本断言。
    assert!(
        all_import_names(&ast).contains(&("os".to_string(), "os".to_string())),
        "DD-0019② 已修复的话删除本断言（模块名不该进 names）"
    );

    // `from a.b import c, d as e` ⇒ 两种名字形状同时在场
    let ast = parse_source(Language::Python, "from a.b import c, d as e\n").expect("from 导入");
    // ⚠ **DD-0019②（这一条把它从"通配专属"扩到"所有 from 导入"）**：实测 names 全集是
    // `[("a.b","a.b"), ("c","c"), ("d","e")]`——模块名 `a.b` 被当成一条**绑定**记进 names，
    // 而 `from a.b import c` 根本不绑定 `a.b`。所以这条泄漏不只在 `from os import *` 形状上，
    // 任何 from 导入都中招（登记原文只写了通配形）。② 修好时 `("a.b","a.b")` 必须消失，
    // 期望改成 `[("c","c"), ("d","e")]` 并删掉本注释——用精确等值而不是子集，正是为了让它红得起来。
    assert_eq!(
        all_import_names(&ast),
        vec![
            ("a.b".to_string(), "a.b".to_string()),
            ("c".to_string(), "c".to_string()),
            ("d".to_string(), "e".to_string())
        ],
        "from 导入当前的 names 全集（含 DD-0019② 的模块名泄漏）"
    );
    assert_eq!(imports(&ast)[0].0, "a.b", "多段模块名要完整落在 module 上");
}

/// `dotted_callee`（Rust）：裸名、`::` 路径、泛型调用三个臂。
#[test]
fn rust_dotted_callee_covers_bare_scoped_and_generic_forms() {
    let ast = parse_source(
        Language::Rust,
        "fn main() {\n    plain();\n    ns::inner();\n    turbo::<u32>();\n}\n",
    )
    .expect("解析 Rust 调用");
    let mut got = callees(&ast);
    got.sort();
    assert_eq!(
        got,
        vec![
            "ns::inner".to_string(),
            "plain".to_string(),
            "turbo".to_string()
        ],
        "裸名 / :: 路径 / 泛型调用都要还原成调用路径（泛型只取函数名，不带尖括号）"
    );
}

/// `::` 路径里的**泛型段要被过滤掉**（`rust_lang.rs` 里那条 `!= "generic_type"` 过滤器）。
/// 只有这种形状才有区分度：过滤臂或过滤器被改坏时，回退臂会给出带尖括号的原文。
#[test]
fn rust_dotted_callee_drops_the_generic_segment_of_a_scoped_path() {
    let ast = parse_source(Language::Rust, "fn main() {\n    Vec::<u32>::new();\n}\n")
        .expect("解析泛型路径调用");
    assert_eq!(
        callees(&ast),
        vec!["new".to_string()],
        "`Vec::<u32>::new()` 要剥掉泛型段（走回退臂会给出 Vec::<u32>::new）"
    );

    // 嵌套调用作接收者：每一层各自成一条 Call，路径不带实参原文
    let ast = parse_source(Language::Rust, "fn main() {\n    x.get(0).unwrap();\n}\n")
        .expect("解析链式调用");
    let got = callees(&ast);
    assert!(
        got.contains(&"x.get".to_string()) && got.contains(&"x.get.unwrap".to_string()),
        "链式调用的每一层都要各自成形，实得：{got:?}"
    );
}

/// `use_tree_paths` + `use_list_items`（Rust）：use 树的四种形状。
/// 绑定名错了，后面的规则/污点面就会以为某个名字没被导入过。
#[test]
fn rust_use_tree_expands_paths_alias_braces_and_wildcard() {
    // 普通路径：绑定名 = 末段
    let ast = parse_source(Language::Rust, "use std::env;\n").expect("解析 use");
    assert_eq!(
        all_import_names(&ast),
        vec![("std::env".to_string(), "env".to_string())]
    );

    // use_as_clause：绑定名 = 别名
    let ast = parse_source(Language::Rust, "use x::y as z;\n").expect("解析 use as");
    assert_eq!(
        all_import_names(&ast),
        vec![("x::y".to_string(), "z".to_string())]
    );

    // scoped_use_list + use_list + use_as_clause：花括号展开，内部别名各自成形
    let ast = parse_source(Language::Rust, "use a::b::{c, d as e};\n").expect("解析花括号 use");
    assert_eq!(
        all_import_names(&ast),
        vec![
            ("a::b::c".to_string(), "c".to_string()),
            ("a::b::d".to_string(), "e".to_string())
        ],
        "花括号里的每一项都要带全路径前缀，别名走 alias"
    );

    // use_wildcard 臂必须被走到。⚠ **DD-0019③**：这里**有意钉住当前的错误行为**——
    // 前缀 `m::n` 整个丢失，只剩 `("::*","*")`（函数文档口径是 `("m::n::*","*")`）。
    // ③ 修好时这条会红，正确处置是把期望改成 ("m::n::*","*")，不是删断言绕过去。
    let wildcard = parse_source(Language::Rust, "use m::n::*;\n").expect("解析通配 use");
    assert_eq!(
        all_import_names(&wildcard),
        vec![("::*".to_string(), "*".to_string())],
        "通配 use 当前的记账形状（精确钉住：多一条、少一条、前缀保住都会在这里红）"
    );
    assert!(
        has_names(&wildcard, &[("::*", "*")]),
        "use_wildcard 臂没走到的话这里会空，实得：{:?}",
        all_import_names(&wildcard)
    );
    // 顶层 `::` 加花括号：展开结果要逐个带上。⚠ 这条**不**钉"独立的 `use_list` 臂被走到"——
    // 实测该形状走的是 `scoped_use_list` 支路，第 2 圈变异门把 `rust_lang.rs:218:9
    // delete match arm "use_list"` 判成存活（删掉那条臂这条断言照样绿），说明没有合法
    // Rust 形状能走到独立臂。留键不硬造夹具，处置随 DD-0019 的冗余臂面一起清。
    let ast = parse_source(Language::Rust, "use ::{root_item};\n").expect("解析顶层花括号");
    assert_eq!(
        all_import_names(&ast),
        vec![("root_item".to_string(), "root_item".to_string())],
        "顶层 `::` 加花括号的每一项都要展开（只钉展开结果，独立臂的可达性见上方说明）"
    );

    // 嵌套花括号：前缀要一路带到底
    let ast =
        parse_source(Language::Rust, "use a::b::{c::{d, e as f}};\n").expect("解析嵌套花括号");
    assert_eq!(
        all_import_names(&ast),
        vec![
            ("a::b::c::d".to_string(), "d".to_string()),
            ("a::b::c::e".to_string(), "f".to_string())
        ],
        "嵌套 use 树的前缀必须逐层累积"
    );

    // ⚠ **DD-0019③**：前缀 `m::n` 当前整个丢失（文档口径是 `("m::n::*","*")`），
    // 通配导入"来自哪个模块"的信息没了。修好时把上面那条改成 ("m::n::*","*") 并删本断言。
    assert!(
        !all_import_names(&ast)
            .iter()
            .any(|(full, _)| full.starts_with("m::n")),
        "DD-0019③ 已修复的话删除本断言（前缀本该保住）"
    );
}
