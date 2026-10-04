//! 解析门面：语言枚举 + 语言分发（tree-sitter → 归一 AST）。

use crate::generic::{AstId, AstNode, GenericAst, Span};
use tree_sitter::Node;

/// 支持的语言（M1 片1：Python + Rust 双语证明 generic AST 抽象）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    /// Python（tree-sitter-python）。
    Python,
    /// Rust（tree-sitter-rust）。
    Rust,
}

#[derive(Debug)]
pub enum ParseError {
    /// tree-sitter 拒绝设置语言/超时等内部错误（解析本身零失败，见 GenericAst::parse_errors）。
    Internal(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Internal(m) => write!(f, "解析器内部错误：{m}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// 解析源码 → 归一 AST。残缺语法不失败（`parse_errors` 计数，RESEARCH 01 零失败纪律）。
pub fn parse_source(lang: Language, src: &str) -> Result<GenericAst, ParseError> {
    let mut parser = tree_sitter::Parser::new();
    let ts_lang = match lang {
        Language::Python => tree_sitter_python::LANGUAGE,
        Language::Rust => tree_sitter_rust::LANGUAGE,
    };
    parser
        .set_language(&ts_lang.into())
        .map_err(|e| ParseError::Internal(e.to_string()))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| ParseError::Internal("tree 为空".into()))?;
    let mut ast = GenericAst::empty();
    let comment_kind = match lang {
        Language::Python => crate::python::COMMENT_KIND,
        Language::Rust => crate::rust_lang::COMMENT_KIND,
    };
    let root = {
        let mut b = Builder { src, ast: &mut ast };
        let root_node = tree.root_node();
        collect_comments(&mut b, root_node, comment_kind);
        let root = match lang {
            Language::Python => crate::python::walk(&mut b, root_node),
            Language::Rust => crate::rust_lang::walk(&mut b, root_node),
        };
        b.ast.root = root;
        root
    };
    ast.root = root;
    ast.comments.sort_unstable_by_key(|(l, _)| *l);
    ast.parse_errors = count_broken(tree.root_node());
    Ok(ast)
}

/// 全树扫 ERROR/MISSING（MISSING 常是无名节点，主遍历的 named_children 碰不到）。
fn count_broken(node: Node) -> usize {
    fn rec(node: Node, n: &mut usize) {
        if node.is_error() || node.is_missing() {
            *n += 1;
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            rec(child, n);
        }
    }
    let mut n = 0;
    rec(node, &mut n);
    n
}

/// 注释预遍历（1-based 行号 → 文本）；主遍历会跳过注释节点。
fn collect_comments(b: &mut Builder, node: Node, comment_kind: &str) {
    if node.kind() == comment_kind {
        let line = node.start_position().row + 1;
        b.note_comment(node, line);
        return;
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        collect_comments(b, ch, comment_kind);
    }
}

/// 构建器：各语言映射共享的底座（文本/区间/压栈/错误与注释记账）。
pub(crate) struct Builder<'a> {
    pub src: &'a str,
    pub ast: &'a mut GenericAst,
}

impl<'a> Builder<'a> {
    pub fn text(&self, node: Node) -> &str {
        &self.src[node.byte_range()]
    }

    /// 驻留节点文本（合并可变借用，规避 text+intern 的双借冲突；先拷贝后查重）。
    pub fn intern_node(&mut self, node: Node) -> u32 {
        let t = self.src[node.byte_range()].to_string();
        self.ast.intern(&t)
    }

    pub fn span(&self, node: Node) -> Span {
        let sp = node.start_position();
        let ep = node.end_position();
        Span {
            start_line: sp.row + 1,
            start_col: sp.column,
            end_line: ep.row + 1,
            end_col: ep.column,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        }
    }

    /// 记录解析错误（ERROR/MISSING），节点折叠为 Other 保留区间与孩子。
    pub fn note_error(&mut self, node: Node) {
        if node.is_error() || node.is_missing() {
            self.ast.parse_errors += 1;
        }
    }

    /// 压栈一个节点并回填孩子指针。
    pub fn push(
        &mut self,
        kind: crate::generic::AstKind,
        span: Span,
        children: Vec<AstId>,
    ) -> AstId {
        let id = AstId(self.ast.nodes.len() as u32);
        for &c in &children {
            self.ast.nodes[c.0 as usize].parent = Some(id);
        }
        self.ast.nodes.push(AstNode {
            kind,
            span,
            children,
            parent: None,
        });
        id
    }

    /// 收集注释（不产 AST 节点）。
    pub fn note_comment(&mut self, node: Node, line: usize) {
        let text = self.text(node).to_string();
        self.ast.comments.push((line, text));
    }
}

/// 展平透传：包装节点返回唯一孩子的句柄（多个/零个孩子时折叠 Other）。
pub(crate) fn flatten(b: &mut Builder, node: Node, span: Span, children: Vec<AstId>) -> AstId {
    if children.len() == 1 {
        return children[0];
    }
    b.push(
        crate::generic::AstKind::Other {
            ts_kind: node.kind().to_string(),
        },
        span,
        children,
    )
}
