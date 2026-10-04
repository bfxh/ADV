//! AST 结构快照（insta）：归一 AST 的形状回归检测——mapper 改动必须过快照审。

use adv_parse::{Language, parse_source};

const PY_FIXTURE: &str =
    "import os\nfrom os import system\ndef h(p):\n    s = input()\n    os.system(s)\n";
const RS_FIXTURE: &str = "use std::env;\nfn main() {\n    let v = env::var(\"X\").unwrap();\n}\n";

#[test]
fn python_ast_snapshot() {
    let ast = parse_source(Language::Python, PY_FIXTURE).unwrap();
    insta::assert_debug_snapshot!("python_ast", ast.nodes);
}

#[test]
fn rust_ast_snapshot() {
    let ast = parse_source(Language::Rust, RS_FIXTURE).unwrap();
    insta::assert_debug_snapshot!("rust_ast", ast.nodes);
}
