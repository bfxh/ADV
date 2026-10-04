//! Rust → 归一 AST 映射（M1 片1）。

use crate::generic::{AstId, AstKind, LiteralKind, Span};
use crate::mapper::{Builder, flatten};
use tree_sitter::Node;

pub const COMMENT_KIND: &str = "line_comment";
const COMMENT_KINDS: &[&str] = &["line_comment", "block_comment"];
const FLATTEN: &[&str] = &["expression_statement", "parenthesized_expression"];

pub fn walk(b: &mut Builder, node: Node) -> AstId {
    let span = b.span(node);
    b.note_error(node);
    if node.kind() == "call_expression" || node.kind() == "macro_invocation" {
        return walk_call(b, node, span);
    }
    let children: Vec<AstId> = node
        .named_children(&mut node.walk())
        .filter(|c| !COMMENT_KINDS.contains(&c.kind()))
        .map(|c| walk(b, c))
        .collect();
    let kind = match node.kind() {
        "source_file" => AstKind::Module,
        "function_item" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| b.text(n).to_string())
                .unwrap_or_default();
            AstKind::FunctionDef {
                name: b.ast.intern(&name),
            }
        }
        "struct_item" | "enum_item" | "union_item" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| b.text(n).to_string())
                .unwrap_or_default();
            AstKind::ClassDef {
                name: b.ast.intern(&name),
            }
        }
        "identifier" => AstKind::Identifier {
            name: b.intern_node(node),
        },
        "field_expression" => {
            let attr = node
                .child_by_field_name("field")
                .map(|n| b.intern_node(n))
                .unwrap_or(0);
            AstKind::Attribute { attr }
        }
        "let_declaration" => {
            let value = children.last().copied();
            let n_targets = children.len().saturating_sub(1);
            AstKind::Assignment {
                targets: children[..n_targets].to_vec(),
                value,
            }
        }
        "if_expression" => AstKind::If,
        "for_expression" => AstKind::For,
        "while_expression" => AstKind::While,
        "return_expression" | "return_statement" => AstKind::Return,
        "use_declaration" => AstKind::Import {
            module: use_path(b, node),
        },
        "string_literal" | "raw_string_literal" => AstKind::Literal {
            kind: LiteralKind::Str,
            text: b.text(node).to_string(),
        },
        "integer_literal" => AstKind::Literal {
            kind: LiteralKind::Int,
            text: b.text(node).to_string(),
        },
        "float_literal" => AstKind::Literal {
            kind: LiteralKind::Float,
            text: b.text(node).to_string(),
        },
        "true" | "false" => AstKind::Literal {
            kind: LiteralKind::Bool,
            text: b.text(node).to_string(),
        },
        "binary_expression" => AstKind::Binary,
        "unary_expression" => AstKind::Unary,
        "block" => AstKind::Block,
        other => AstKind::Other {
            ts_kind: other.to_string(),
        },
    };
    if FLATTEN.contains(&node.kind()) {
        return flatten(b, node, span, children);
    }
    b.push(kind, span, children)
}

/// 调用/宏调用：点分被调名（`foo` / `std::io::foo` / `x.unwrap` / `panic!`）。
fn walk_call(b: &mut Builder, node: Node, span: Span) -> AstId {
    let fn_field = if node.kind() == "macro_invocation" {
        "macro"
    } else {
        "function"
    };
    let fn_node = node.child_by_field_name(fn_field);
    let mut callee = dotted_callee(b, fn_node);
    let mut children = Vec::new();
    if let Some(f) = fn_node
        && !COMMENT_KINDS.contains(&f.kind())
    {
        children.push(walk(b, f));
    }
    let mut args = Vec::new();
    if let Some(args_node) = node.child_by_field_name("arguments") {
        let mut c = args_node.walk();
        for a in args_node.named_children(&mut c) {
            if COMMENT_KINDS.contains(&a.kind()) {
                continue;
            }
            let aid = walk(b, a);
            args.push(aid);
            children.push(aid);
        }
    }
    if node.kind() == "macro_invocation" {
        callee.push('!');
    }
    b.push(
        AstKind::Call {
            callee,
            args,
            kwargs: Vec::new(),
        },
        span,
        children,
    )
}

/// 点分被调名（Rust 路径分隔 `::`；字段访问 `.`）。
fn dotted_callee(b: &mut Builder, node: Option<Node>) -> String {
    let Some(n) = node else { return String::new() };
    match n.kind() {
        "identifier" => b.text(n).to_string(),
        // 嵌套调用作接收者（x.get(0).unwrap()）时只取其函数路径，不带实参原文
        "call_expression" | "macro_invocation" => {
            dotted_callee(b, n.child_by_field_name("function"))
        }
        "field_expression" => {
            let obj = dotted_callee(b, n.child_by_field_name("value"));
            let attr = n
                .child_by_field_name("field")
                .map(|c| b.text(c).to_string())
                .unwrap_or_default();
            if obj.is_empty() {
                attr
            } else {
                format!("{obj}.{attr}")
            }
        }
        "scoped_identifier" | "scoped_type_identifier" => {
            let mut c = n.walk();
            let parts: Vec<String> = n
                .named_children(&mut c)
                .filter(|p| p.kind() != "generic_type" && p.kind() != COMMENT_KINDS[0])
                .map(|p| match p.kind() {
                    "identifier" => b.text(p).to_string(),
                    _ => dotted_callee(b, Some(p)),
                })
                .collect();
            parts.join("::")
        }
        "generic_function" => dotted_callee(b, n.child_by_field_name("function")),
        _ => b.text(n).to_string(),
    }
}

fn use_path(b: &mut Builder, node: Node) -> String {
    node.child_by_field_name("argument")
        .map(|a| b.text(a).to_string())
        .unwrap_or_default()
}
