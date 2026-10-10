//! 增量索引层（M4-4a + M4-4b）：以"整份文档集合"为输入，只重算内容真的变了的那几篇，
//! 并把"原文 + tf 表"整份落盘，跨进程重启后仍然只做字节比对。
//!
//! 增量买到的东西很具体：**跳过未变更文档的切词**（`tokenize` 是这条链上最贵的一段）。
//! 比对用**逐字节相等**而不是概率性哈希。蓝图 §12 E2 写的是"保守上界的哈希 + 便宜复验"，
//! 这里**不照抄**，理由是量出来的而不是偏好：2404 篇 / 111,777 token 的逐字节比对实测
//! **3.8 ms**（见 `bench/retrieval/incremental_cost.py` 的 `warm_apply`），比任何指纹方案都便宜，
//! 而且不带概率项——哈希方案要额外背"采样复验仍有未采样漏网"这条残余风险，省下的只是磁盘体积。
//! 代价也写清楚：缓存里存了原文副本，**体积约等于再存一份语料**，这是"零概率项"的价格。
//!
//! 倒排本身每次都从缓存的 tf 表重建（代价随**不同词数**走，不随 token 数走），
//! 所以这层的复用收益全部落在"少切了几篇"上，别读成"少建了索引"。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::bm25::{SearchIndex, tokenize};

/// 磁盘缓存的格式版本；[`IndexStore::from_json`] 认不出的版本一律判 `Err`（调用方重建，不猜）。
pub const CACHE_VERSION: u32 = 1;

/// 一篇文档的缓存单元：原文 + 已算好的 tf 表。
struct Cached {
    /// 原文；复用的唯一依据是它与新输入**逐字节相等**。
    text: String,
    /// `tokenize(text)` 的计数结果。
    term_counts: HashMap<String, u32>,
}

/// 落盘形状的一篇：`terms` 出盘前按词字典序排、`docs` 按 id 排 ⇒ **同一批文档两次导出逐字节相同**
/// （这条自证写在 `tests/disk_cache.rs`，它是"缓存文件可 diff、可钉哈希"的前提）。
#[derive(Serialize, Deserialize)]
struct DiskDoc {
    id: String,
    text: String,
    terms: Vec<(String, u32)>,
}

/// 落盘形状：版本号 + 文档顺序 + 逐篇（顺序与 `docs` 分开存，因为顺序是打分并列-break 的一部分）。
#[derive(Serialize, Deserialize)]
struct DiskCache {
    version: u32,
    order: Vec<String>,
    docs: Vec<DiskDoc>,
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

    /// 累计喂进 `tokenize` 的 token 数：复用生效时这个数**不涨**。
    pub fn tokens_tokenized(&self) -> u64 {
        self.tokens_tokenized
    }

    /// 累计切过的篇数（新增 + 变更）。
    pub fn retokenized(&self) -> usize {
        self.retokenized
    }

    /// 导出为 JSON 文本（**确定序**：`docs` 按 id、每篇 `terms` 按词排序）。
    ///
    /// 因此同一批文档两次导出的字节完全相同 ⇒ 缓存文件可以 diff、可以钉哈希、可以在门里做
    /// "重存必须逐字节相同"的自证。
    pub fn to_json(&self) -> Result<String, String> {
        let mut docs: Vec<DiskDoc> = self
            .cached
            .iter()
            .map(|(id, c)| {
                let mut terms: Vec<(String, u32)> =
                    c.term_counts.iter().map(|(t, n)| (t.clone(), *n)).collect();
                terms.sort();
                DiskDoc {
                    id: id.clone(),
                    text: c.text.clone(),
                    terms,
                }
            })
            .collect();
        docs.sort_by(|left, right| left.id.cmp(&right.id));
        serde_json::to_string(&DiskCache {
            version: CACHE_VERSION,
            order: self.order.clone(),
            docs,
        })
        .map_err(|error| format!("缓存导出失败：{error}"))
    }

    /// 从 JSON 文本读回。计量口径（`tokens_tokenized`/`retokenized`）**从 0 起**——那是本进程的账，
    /// 不跨进程继承，否则"热轮零重切词"那条自检会被读回的旧值糊过去。
    ///
    /// 版本不符、id 重复、`order` 引用了不存在的 id ⇒ 一律 `Err`（调用方重建，不猜、不半接）。
    pub fn from_json(text: &str) -> Result<Self, String> {
        let cache: DiskCache =
            serde_json::from_str(text).map_err(|error| format!("缓存不是本格式：{error}"))?;
        if cache.version != CACHE_VERSION {
            return Err(format!("缓存版本 {} ≠ 当前 {CACHE_VERSION}", cache.version));
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut cached: HashMap<String, Cached> = HashMap::new();
        for doc in cache.docs {
            if !seen.insert(doc.id.clone()) {
                return Err(format!("缓存里 id 重复：{}", doc.id));
            }
            let count = doc.terms.len();
            let term_counts: HashMap<String, u32> = doc.terms.into_iter().collect();
            if term_counts.len() != count {
                return Err(format!("缓存里 {} 的词表有重复项", doc.id));
            }
            cached.insert(
                doc.id,
                Cached {
                    text: doc.text,
                    term_counts,
                },
            );
        }
        for id in &cache.order {
            if !cached.contains_key(id) {
                return Err(format!("缓存的文档顺序引用了不存在的 id：{id}"));
            }
        }
        Ok(Self {
            cached,
            order: cache.order,
            tokens_tokenized: 0,
            retokenized: 0,
        })
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
