//! 增量索引层（M4-4a）：以"整份文档集合"为输入，只重算内容真的变了的那几篇。
//!
//! 增量买到的东西很具体：**跳过未变更文档的切词**（`tokenize` 是这条链上最贵的一段）。
//! 每篇缓存同时存原文与 `term → tf` 表，比对用**逐字节相等**而不是概率性哈希——
//! 判据要能当场说清（"指纹没变"带概率，"字节没变"不带），而 M4-4 的验收要求增量产物
//! 与全量重建**相等**，这里不能塞一个概率项。磁盘化缓存（只存 tf 表不存原文）留后续片，
//! 那片才需要"保守上界的哈希 + 便宜复验"这套（蓝图 §12 E2）。
//!
//! 倒排本身每次都从缓存的 tf 表重建（代价随**不同词数**走，不随 token 数走），
//! 所以这层的复用收益全部落在"少切了几篇"上，别读成"少建了索引"。

use std::collections::{HashMap, HashSet};

use crate::bm25::{SearchIndex, tokenize};

/// 一篇文档的缓存单元：原文 + 已算好的 tf 表。
struct Cached {
    /// 原文；复用的唯一依据是它与新输入**逐字节相等**。
    text: String,
    /// `tokenize(text)` 的计数结果。
    term_counts: HashMap<String, u32>,
}

/// 一次 [`IndexStore::apply`] 的账面：四类文档各多少篇。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Delta {
    /// 缓存里没有 ⇒ 新切词的篇数。
    pub added: usize,
    /// 缓存里有但字节不同 ⇒ 重切词的篇数。
    pub changed: usize,
    /// 字节没变 ⇒ 直接复用 tf 表的篇数。
    pub reused: usize,
    /// 缓存里有但这次输入没有 ⇒ 掉出索引的篇数。
    pub dropped: usize,
}

/// 增量索引：持有缓存集合，每次 `apply` 按输入整份对齐。
pub struct IndexStore {
    /// `doc_id → 缓存单元`。
    cached: HashMap<String, Cached>,
    /// 当前文档顺序（= 最近一次 `apply` 的输入顺序）。
    order: Vec<String>,
    /// 累计真正喂进 `tokenize` 的 token 数（复用收益的计量口径）。
    tokens_tokenized: u64,
    /// 累计切过的篇数（新增 + 变更）。
    retokenized: usize,
}

impl IndexStore {
    /// 空缓存：第一次 `apply` 的账全落在 `added`。
    pub fn new() -> Self {
        Self {
            cached: HashMap::new(),
            order: Vec::new(),
            tokens_tokenized: 0,
            retokenized: 0,
        }
    }

    /// 把输入当作**新的完整文档集合**对齐进缓存，返回四类计数。
    ///
    /// 同 id 且逐字节相同 ⇒ 复用（一次 `tokenize` 都不做）；否则整篇重算。
    /// 输入里出现重复 id ⇒ `Err` 且**缓存一字不动**（与 `adv-sca` 快照同一条纪律：
    /// 不把"改了一半"记成"同步过了"）——评测与记账都假定 id 唯一。
    pub fn apply(&mut self, docs: Vec<(String, String)>) -> Result<Delta, String> {
        let mut unique: HashSet<&str> = HashSet::with_capacity(docs.len());
        for (id, _text) in &docs {
            if !unique.insert(id.as_str()) {
                return Err(format!("文档 id 重复：{id}"));
            }
        }

        let mut delta = Delta::default();
        let mut order = Vec::with_capacity(docs.len());
        let mut live: HashSet<String> = HashSet::with_capacity(docs.len());
        for (id, text) in docs {
            live.insert(id.clone());
            match self.cached.get(&id) {
                Some(hit) if hit.text == text => delta.reused += 1,
                Some(_) => {
                    let term_counts = Self::count(&text, &mut self.tokens_tokenized);
                    self.cached.insert(id.clone(), Cached { text, term_counts });
                    self.retokenized += 1;
                    delta.changed += 1;
                }
                None => {
                    let term_counts = Self::count(&text, &mut self.tokens_tokenized);
                    self.cached.insert(id.clone(), Cached { text, term_counts });
                    self.retokenized += 1;
                    delta.added += 1;
                }
            }
            order.push(id);
        }
        delta.dropped = self.order.iter().filter(|id| !live.contains(*id)).count();
        self.cached.retain(|id, _| live.contains(id));
        self.order = order;
        Ok(delta)
    }

    /// 用当前缓存的 tf 表建索引（这一步一篇都不切词）。
    pub fn index(&self) -> SearchIndex {
        SearchIndex::build_from_stats(self.order.iter().filter_map(|id| {
            self.cached
                .get(id)
                .map(|c| (id.clone(), c.term_counts.clone()))
        }))
    }

    /// 在当前缓存集合上查询（与 [`IndexStore::index`] 同一条打分路径）。
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        self.index().search(query, limit)
    }

    /// 累计喂进 `tokenize` 的 token 数：复用生效时这个数**不涨**。
    pub fn tokens_tokenized(&self) -> u64 {
        self.tokens_tokenized
    }

    /// 累计切过的篇数（新增 + 变更）。
    pub fn retokenized(&self) -> usize {
        self.retokenized
    }

    fn count(text: &str, tokens: &mut u64) -> HashMap<String, u32> {
        let mut term_counts: HashMap<String, u32> = HashMap::new();
        for token in tokenize(text) {
            *tokens += 1;
            *term_counts.entry(token).or_default() += 1;
        }
        term_counts
    }
}

impl Default for IndexStore {
    fn default() -> Self {
        Self::new()
    }
}
