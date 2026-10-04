//! ADV 解析层（adv-parse）。
//!
//! 选型已定（RESEARCH 01/09、X1、D10）：tree-sitter 0.25 多语言主解析（零失败解析，
//! 残缺代码照常产出 AST）；自建最小归一 AST（Semgrep「CST→generic AST」形状）；
//! 句柄化存储 = 平铺 Vec + AstId(u32)（X1：不可删集合用 id-arena 形状，自建零依赖等价物）。
//! M1 片1 落 Python + Rust 双语映射；salsa 增量（片2）、ropey 缓冲（片3）随后。

mod generic;
mod mapper;
mod python;
mod rust_lang;

pub use generic::{AstId, AstKind, AstNode, GenericAst, LiteralKind, Span};
pub use mapper::{Language, parse_source};

/// 域版本（与工作区同源）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_core() {
        assert_eq!(super::version(), adv_core::version());
    }

    #[test]
    fn python_call_callee_is_dotted() {
        let ast = parse_source(Language::Python, "import os\nos.system('ls')\n").unwrap();
        let calls =
            ast.find(|n| matches!(&n.kind, AstKind::Call { callee, .. } if callee == "os.system"));
        assert_eq!(calls.len(), 1, "应找到 os.system 调用");
        assert_eq!(ast.parse_errors, 0);
    }

    #[test]
    fn rust_call_and_fn() {
        let src = "fn main() { foo(1); }\n";
        let ast = parse_source(Language::Rust, src).unwrap();
        assert!(
            ast.find(|n| matches!(&n.kind, AstKind::FunctionDef { .. }))
                .len()
                == 1,
            "应找到 fn main"
        );
        assert!(
            ast.find(|n| matches!(&n.kind, AstKind::Call { callee, .. } if callee == "foo"))
                .len()
                == 1,
            "应找到 foo 调用"
        );
    }

    #[test]
    fn broken_syntax_still_yields_ast() {
        // 零失败解析：残缺代码不许 panic，不许零 AST（RESEARCH 01 rowan 纪律）
        let ast = parse_source(Language::Python, "def broken(:\n  pass\n").unwrap();
        assert!(!ast.nodes.is_empty());
        assert!(ast.parse_errors >= 1, "残缺语法应计错误数");
    }
}
