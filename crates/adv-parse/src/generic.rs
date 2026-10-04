//! 归一 AST（generic AST）：语言无关的最小结构层（蓝图 §4；RESEARCH 01「CST→generic AST」形状）。
//!
//! 设计（RESEARCH X1）：`Vec<AstNode>` 平铺 + `AstId(u32)` 句柄（不可删集合，重建式更新）；
//! 标识符/字面量文本驻留在 `GenericAst::intern`。句柄 Copy，规则层不持有 tree-sitter 类型。

/// 节点句柄（平铺数组下标；Copy 是规则层遍历的承重前提）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AstId(pub u32);

/// 源码区间（1-based 行、0-based 列，UTF-8 码点口径；字节偏移供预过滤层用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// 起始行（1-based）。
    pub start_line: usize,
    /// 起始列（0-based，UTF-8 码点）。
    pub start_col: usize,
    /// 结束行（1-based，含）。
    pub end_line: usize,
    /// 结束列（0-based，不含）。
    pub end_col: usize,
    /// 起始字节偏移（预过滤层消费）。
    pub start_byte: usize,
    /// 结束字节偏移。
    pub end_byte: usize,
}

/// 语言无关的节点种类（M1 片1：结构匹配所需的最小闭集；未映射节点折叠为 `Other`）。
#[derive(Clone, Debug, PartialEq)]
pub enum AstKind {
    /// 编译单元根（module/source_file）。
    Module,
    /// 函数定义（name = 驻留名下标）。
    FunctionDef {
        /// 驻留名下标。
        name: u32,
    },
    /// 类型定义（class/struct/enum/union 统一口径）。
    ClassDef {
        /// 驻留名下标。
        name: u32,
    },
    /// 调用（含宏调用 `name!`）。
    Call {
        /// 点分调用名（如 `os.system`）；解析不出时为空串。
        callee: String,
        /// 位置参数。
        args: Vec<AstId>,
        /// 关键字参数（Python）。
        kwargs: Vec<(String, AstId)>,
    },
    /// 赋值/let 绑定。
    Assignment {
        /// 赋值目标（可多个，Python 链式）。
        targets: Vec<AstId>,
        /// 右值。
        value: Option<AstId>,
    },
    /// 标识符（name = 驻留下标）。
    Identifier {
        /// 驻留名下标。
        name: u32,
    },
    /// 成员访问（a.b / a::b 的叶子名）。
    Attribute {
        /// 驻留叶子名下标。
        attr: u32,
    },
    /// 字面量（原文含引号/前缀）。
    Literal {
        /// 字面量类别。
        kind: LiteralKind,
        /// 原文。
        text: String,
    },
    /// 条件分支。
    If,
    /// 循环（for）。
    For,
    /// 循环（while）。
    While,
    /// 返回语句。
    Return,
    /// 导入（供导入感知解析：`from X import a as b` ⇒ b → X.a）。
    Import {
        /// from-来源模块（`import a.b` 时为空串）。
        module: String,
        /// (被导入名, 绑定别名)；`import a.b as c` 记 ("a.b","c")。
        names: Vec<(String, String)>,
    },
    /// 二元运算。
    Binary,
    /// 一元运算。
    Unary,
    /// 比较（Python 专用节点；Rust 归 Binary）。
    Compare,
    /// 语句块。
    Block,
    /// 未映射节点：保留 tree-sitter 原始 kind（零失败解析的兜底面）。
    Other {
        /// 原始节点 kind。
        ts_kind: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 字面量类别。
pub enum LiteralKind {
    /// 字符串。
    Str,
    /// 整数。
    Int,
    /// 浮点。
    Float,
    /// 布尔。
    Bool,
    /// 其他（字节串等）。
    Other,
}

/// 归一 AST 节点。
#[derive(Clone, Debug)]
pub struct AstNode {
    /// 节点种类。
    pub kind: AstKind,
    /// 源码区间。
    pub span: Span,
    /// 孩子句柄（有序）。
    pub children: Vec<AstId>,
    /// 父句柄（根为 None）。
    pub parent: Option<AstId>,
}

/// 一棵归一 AST（每文件一棵；驻留表共享）。
#[derive(Clone, Debug)]
pub struct GenericAst {
    /// 平铺节点表（AstId = 下标）。
    pub nodes: Vec<AstNode>,
    /// 根节点。
    pub root: AstId,
    strings: Vec<String>,
    /// tree-sitter 解析残留的 ERROR/MISSING 节点数（0 = 完整解析；残缺代码照常产出 AST）。
    /// tree-sitter ERROR/MISSING 计数（0 = 完整解析）。
    pub parse_errors: usize,
    /// 注释（1-based 行号 → 文本），供规则测试门禁（ruleid 注释）消费。
    pub comments: Vec<(usize, String)>,
}

impl GenericAst {
    /// 空表（解析入口用；字段私有，全部经方法访问）。
    pub fn empty() -> Self {
        GenericAst {
            nodes: Vec::new(),
            root: AstId(0),
            strings: Vec::new(),
            parse_errors: 0,
            comments: Vec::new(),
        }
    }

    /// 取节点（句柄由本表发出，恒有效）。
    pub fn get(&self, id: AstId) -> &AstNode {
        &self.nodes[id.0 as usize]
    }

    /// 驻留字符串（标识符/字面量文本），返回下标作句柄。
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(i) = self.strings.iter().position(|x| x == s) {
            return i as u32;
        }
        self.strings.push(s.to_string());
        (self.strings.len() - 1) as u32
    }

    /// 取回驻留字符串。
    pub fn string(&self, idx: u32) -> &str {
        &self.strings[idx as usize]
    }

    /// 首个匹配谓词的节点（片1 的朴素查询面；索引/增量留片2）。
    pub fn find(&self, pred: impl Fn(&AstNode) -> bool) -> Vec<AstId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| pred(n))
            .map(|(i, _)| AstId(i as u32))
            .collect()
    }
}
