//! Python → 归一 AST 映射（M1 片1）。

use crate::generic::{AstId, AstKind, LiteralKind, Span};
use crate::mapper::Builder;
use tree_sitter::Node;

pub const COMMENT_KIND: &str = "comment";
const FLATTEN: &[&str] = &["expression_statement", "parenthesized_expression"];

pub fn walk(b: &mut Builder, node: Node) -> AstId {
    let span = b.span(node);
    b.note_error(node);
    // call 特判：孩子不走通用收集（function 路径 + args/kwargs 各自建账，防重复占位）
    if node.kind() == "call" {
        return walk_call(b, node, span);
    }
    let children: Vec<AstId> = node
        .named_children(&mut node.walk())
        .filter(|c| c.kind() != COMMENT_KIND)
        .map(|c| walk(b, c))
        .collect();
    let kind = match node.kind() {
        "module" => AstKind::Module,
        "function_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| b.text(n).to_string())
                .unwrap_or_default();
            AstKind::FunctionDef {
                name: b.ast.intern(&name),
            }
        }
        "class_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| b.text(n).to_string())
                .unwrap_or_default();
            AstKind::ClassDef {
                name: b.ast.intern(&name),
            }
        }
        "assignment" => {
            let value = children.last().copied();
            let n_targets = children.len().saturating_sub(1);
            AstKind::Assignment {
                targets: children[..n_targets].to_vec(),
                value,
            }
        }
        "identifier" => AstKind::Identifier {
            name: b.intern_node(node),
        },
        "attribute" => {
            let attr = node
                .child_by_field_name("attribute")
                .map(|n| b.intern_node(n))
                .unwrap_or(0);
            AstKind::Attribute { attr }
        }
        "if_statement" => AstKind::If,
        "for_statement" => AstKind::For,
        "while_statement" => AstKind::While,
        "return_statement" => AstKind::Return,
        "import_statement" | "import_from_statement" => import_kind(b, node),
        "string" => AstKind::Literal {
            kind: LiteralKind::Str,
            text: b.text(node).to_string(),
        },
        "integer" => AstKind::Literal {
            kind: LiteralKind::Int,
            text: b.text(node).to_string(),
        },
        "float" => AstKind::Literal {
            kind: LiteralKind::Float,
            text: b.text(node).to_string(),
        },
        "true" | "false" => AstKind::Literal {
            kind: LiteralKind::Bool,
            text: b.text(node).to_string(),
        },
        "binary_operator" => AstKind::Binary,
        "unary_operator" => AstKind::Unary,
        "comparison_operator" => AstKind::Compare,
        "block" => AstKind::Block,
        other => AstKind::Other {
            ts_kind: other.to_string(),
        },
    };
    if FLATTEN.contains(&node.kind()) {
        return crate::mapper::flatten(b, node, span, children);
    }
    b.push(kind, span, children)
}

/// 调用节点：点分被调名 + 位置参数 + 关键字参数（孩子 = function 路径 + 全部实参）。
fn walk_call(b: &mut Builder, node: Node, span: Span) -> AstId {
    let callee = dotted_callee(b, node.child_by_field_name("function"));
    let mut children = Vec::new();
    if let Some(fn_node) = node.child_by_field_name("function")
        && fn_node.kind() != COMMENT_KIND
    {
        children.push(walk(b, fn_node));
    }
    let mut args = Vec::new();
    let mut kwargs = Vec::new();
    if let Some(args_node) = node.child_by_field_name("arguments") {
        let mut c = args_node.walk();
        for a in args_node.named_children(&mut c) {
            if a.kind() == "keyword_argument" {
                let kw = a
                    .child_by_field_name("name")
                    .map(|n| b.text(n).to_string())
                    .unwrap_or_default();
                if let Some(v) = a.child_by_field_name("value") {
                    let vid = walk(b, v);
                    kwargs.push((kw, vid));
                    children.push(vid);
                }
            } else {
                let aid = walk(b, a);
                args.push(aid);
                children.push(aid);
            }
        }
    }
    b.push(
        AstKind::Call {
            callee,
            args,
            kwargs,
        },
        span,
        children,
    )
}

/// 点分被调名：`os.system` / `a.b.c`（纯文本拼装；路径节点仍照常入 AST）。
fn dotted_callee(b: &mut Builder, node: Option<Node>) -> String {
    let Some(n) = node else { return String::new() };
    match n.kind() {
        "identifier" => b.text(n).to_string(),
        "attribute" => {
            let obj = dotted_callee(b, n.child_by_field_name("object"));
            let attr = n
                .child_by_field_name("attribute")
                .map(|c| b.text(c).to_string())
                .unwrap_or_default();
            if obj.is_empty() {
                attr
            } else {
                format!("{obj}.{attr}")
            }
        }
        _ => b.text(n).to_string(),
    }
}

/// 导入建账：`import a.b [as c]` / `from m import x [as y]` ⇒ (被导入名, 绑定名) 对。
fn import_kind(b: &mut Builder, node: Node) -> AstKind {
    let is_from = node.kind() == "import_from_statement";
    let module = if is_from {
        node.child_by_field_name("module_name")
            .map(|m| b.text(m).to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut names = Vec::new();
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        match ch.kind() {
            "aliased_import" => {
                let base = ch
                    .child_by_field_name("name")
                    .map(|n| b.text(n).to_string())
                    .unwrap_or_default();
                let alias = ch
                    .child_by_field_name("alias")
                    .map(|n| b.text(n).to_string())
                    .unwrap_or_else(|| base.clone());
                names.push((base, alias));
            }
            "dotted_name" if !is_from => {
                let t = b.text(ch).to_string();
                let bind = t.split('.').next().unwrap_or("").to_string();
                if bind == t {
                    // `import os`：绑定名即路径本身（恒等，不需映射）；
                    // `import a.b`（无别名）绑定根段 a ⇒ 调用 a.b 本就按路径解析，
                    // 不入映射（避免 a→a.b 的首段替换拼出 a.b.b）。
                    continue;
                }
                names.push((t, bind));
            }
            "name" | "dotted_name" if is_from => {
                let t = b.text(ch).to_string();
                names.push((t.clone(), t));
            }
            "wildcard_import" => names.push(("*".into(), "*".into())),
            _ => {}
        }
    }
    AstKind::Import { module, names }
}
