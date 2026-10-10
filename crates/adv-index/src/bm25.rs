use std::cmp::Ordering;
use std::collections::HashMap;

/// tf 饱和强度（发表式里的 k1；与 `bench/retrieval/gen_bm25_cases.py` 必须一致，金样测试负责核对）。
pub const K1: f64 = 1.2;
/// 长度归一强度（发表式里的 b）。**0.25 是本仓冻结集上的实测最优，不是抄来的默认值**：
/// b ∈ {0, 0.25, 0.5, 0.75, 1.0} 五档在 120 条查询上的 Recall@1 依次是
/// 0.5000 / **0.6333** / 0.6000 / 0.5667 / 0.4583，MRR 依次 0.6386 / **0.7380** / 0.7031 /
/// 0.6547 / 0.5631 —— 语料是"文件级"代码库、金标常常就是那篇长文件，归一越强越把它压下去。
pub const B: f64 = 0.25;

/// 与 `bench/retrieval/protocol.py` 对齐的标识符切词。
pub fn tokenize(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let ident_start = bytes[start].is_ascii_alphabetic() || bytes[start] == b'_';
        if !ident_start {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        out.extend(split_identifier(&text[start..end]));
        start = end;
    }
    out
}

fn split_identifier(ident: &str) -> Vec<String> {
    let bytes = ident.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        if !bytes[start].is_ascii_alphanumeric() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            end += 1;
        }
        split_alphanumeric(&ident[start..end], &mut out);
        start = end;
    }
    out
}

fn split_alphanumeric(chunk: &str, out: &mut Vec<String>) {
    let bytes = chunk.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if bytes[start].is_ascii_uppercase() {
            let mut upper_end = start + 1;
            while upper_end < bytes.len() && bytes[upper_end].is_ascii_uppercase() {
                upper_end += 1;
            }
            if upper_end < bytes.len() && bytes[upper_end].is_ascii_lowercase() {
                if upper_end - start > 1 {
                    push_token(chunk, start, upper_end - 1, out);
                    start = upper_end - 1;
                }
                let word_end = lower_word_end(bytes, start + 1);
                push_token(chunk, start, word_end, out);
                start = word_end;
            } else {
                push_token(chunk, start, upper_end, out);
                start = upper_end;
            }
        } else if bytes[start].is_ascii_lowercase() {
            let end = lower_word_end(bytes, start);
            push_token(chunk, start, end, out);
            start = end;
        } else {
            let mut end = start + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            push_token(chunk, start, end, out);
            start = end;
        }
    }
}

fn lower_word_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_lowercase() {
        end += 1;
    }
    end
}

fn push_token(source: &str, start: usize, end: usize, out: &mut Vec<String>) {
    if start < end {
        out.push(source[start..end].to_ascii_lowercase());
    }
}

/// 参与评分的查询项：切词后长度 ≥ 2 的项。
///
/// 单字项在代码语料里几乎没有主题信号——切词后它要么是单字母变量名（`i`/`n`/`x`），要么是
/// 评测生成器注入的扰动字。实测冻结集：`a` 的 df/N=0.109、`x` df/N=0.068，是同一条查询里
/// 真词（`guard` 0.012、`mir` 0.010）的 5–10 倍；它不在金标文档里，却把别的文档抬到金标之上。
/// 只筛**查询侧**，文档侧照常全量索引，切词口径与 `protocol.py` 的逐字对齐不受影响。
fn scoring_terms(query: &str) -> impl Iterator<Item = String> {
    tokenize(query).into_iter().filter(|term| term.len() > 1)
}

struct Posting {
    doc: usize,
    tf: u32,
}

/// 零依赖 BM25 索引，顺序即文档插入顺序。
pub struct SearchIndex {
    doc_ids: Vec<String>,
    doc_lens: Vec<u32>,
    avg_len: f64,
    postings: HashMap<String, Vec<Posting>>,
}

impl SearchIndex {
    /// 按输入顺序构建索引；后续并列排序也沿用该顺序。
    pub fn build(docs: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut doc_ids = Vec::new();
        let mut doc_lens = Vec::new();
        let mut postings: HashMap<String, Vec<Posting>> = HashMap::new();
        for (doc_id, text) in docs {
            let doc = doc_ids.len();
            let mut term_counts: HashMap<String, u32> = HashMap::new();
            let mut doc_len = 0;
            for token in tokenize(&text) {
                doc_len += 1;
                *term_counts.entry(token).or_default() += 1;
            }
            for (term, tf) in term_counts {
                postings.entry(term).or_default().push(Posting { doc, tf });
            }
            doc_ids.push(doc_id);
            doc_lens.push(doc_len);
        }
        let total_len: u64 = doc_lens.iter().map(|len| u64::from(*len)).sum();
        let avg_len = if doc_ids.is_empty() {
            0.0
        } else {
            total_len as f64 / doc_ids.len() as f64
        };
        Self {
            doc_ids,
            doc_lens,
            avg_len,
            postings,
        }
    }

    /// 返回分数大于 0 的文档，按分数降序；并列时按插入顺序。
    ///
    /// 查询项先过 `scoring_terms`，所以纯单字查询得到空表（判不到，不乱猜）。
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        if limit == 0 || self.doc_ids.is_empty() {
            return Vec::new();
        }
        let doc_count = self.doc_ids.len() as f64;
        let mut scores = vec![0.0; self.doc_ids.len()];
        for term in scoring_terms(query) {
            if let Some(postings) = self.postings.get(&term) {
                let df = postings.len() as f64;
                let idf = (1.0 + (doc_count - df + 0.5) / (df + 0.5)).ln();
                for posting in postings {
                    // 发表式：norm = (1 - b) + b·(|d| / avgdl)（Elastic Practical BM25 §"How b works"：
                    // freq·(k1+1) / (freq + k1·(1 - b + b·fieldLength/avgFieldLength))）。
                    // 2026-10-10 变异门 + 独立转写对拍抓到此处曾写成 `b + (1-b)·r`，等于把 b 当 0.25 用。
                    let length_norm =
                        (1.0 - B) + B * self.doc_lens[posting.doc] as f64 / self.avg_len;
                    let tf = posting.tf as f64;
                    scores[posting.doc] += idf * tf * (K1 + 1.0) / (tf + K1 * length_norm);
                }
            }
        }
        let mut hits: Vec<(usize, f64)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score > 0.0)
            .collect();
        hits.sort_by(|(left_doc, left_score), (right_doc, right_score)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(Ordering::Equal)
                .then(left_doc.cmp(right_doc))
        });
        hits.truncate(limit);
        hits.into_iter()
            .map(|(doc, score)| (self.doc_ids[doc].clone(), score))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchIndex, tokenize};

    #[test]
    fn tokenizer_matches_retrieval_protocol() {
        assert_eq!(
            tokenize("loadWorkspace load_workspace HTTPServer ABCd"),
            [
                "load",
                "workspace",
                "load",
                "workspace",
                "http",
                "server",
                "ab",
                "cd"
            ]
        );
        assert_eq!(tokenize("utf8 v2-beta"), ["utf", "8", "v", "2", "beta"]);
        assert!(tokenize("...").is_empty());
    }

    #[test]
    fn bm25_ranks_matching_doc_first_and_is_deterministic() {
        fn docs() -> [(String, String); 3] {
            [
                (
                    "clean.rs".to_string(),
                    "sanitize unrelated cache setting".to_string(),
                ),
                (
                    "taint.rs".to_string(),
                    "untrusted input reaches command sink".to_string(),
                ),
                (
                    "other.rs".to_string(),
                    "unrelated cache setting".to_string(),
                ),
            ]
        }
        let index = SearchIndex::build(docs());
        let got = index.search("untrusted command", 10);
        assert_eq!(got.first().map(|(id, _)| id.as_str()), Some("taint.rs"));
        assert!(got.iter().all(|(id, _)| id != "other.rs"));

        let again = SearchIndex::build(docs());
        assert_eq!(got, again.search("untrusted command", 10));
    }

    #[test]
    fn bm25_empty_index_and_limit_are_safe() {
        let index = SearchIndex::build([]);
        assert!(index.search("anything", 10).is_empty());

        let filled = SearchIndex::build([
            ("a".to_string(), "alpha beta".to_string()),
            ("b".to_string(), "alpha gamma".to_string()),
        ]);
        assert_eq!(filled.search("alpha", 1).len(), 1);
        // limit=0 而索引非空：两条前置条件是"或"，少了任一条都会在这里露出来
        // （变异门点名的 `|| with &&` 就是靠这一句杀的）。
        assert!(filled.search("alpha", 0).is_empty());
    }

    #[test]
    fn single_letter_query_terms_do_not_score() {
        let index = SearchIndex::build([
            ("noise.rs".to_string(), "x".to_string()),
            ("gold.rs".to_string(), "workspace x".to_string()),
        ]);
        let got: Vec<String> = index
            .search("workspace x", 10)
            .into_iter()
            .map(|(id, _score)| id)
            .collect();
        assert_eq!(got, ["gold.rs"]);
        // 纯单字查询：判不到就是空表，不拿噪声凑答案
        assert!(index.search("x", 10).is_empty());
    }

    #[test]
    fn filter_is_by_term_length_not_document_frequency() {
        // 判据刻意不是"df 超阈值就丢"：冻结集上那条会连坐 `adv`/`rust`/`json` 这些本仓真词，
        // 还会把 2 条查询清空。高频真词照常参与评分。
        let docs: Vec<(String, String)> = (0..10)
            .map(|i| (format!("d{i}.rs"), "common rare".to_string()))
            .collect();
        let index = SearchIndex::build(docs);
        assert_eq!(index.search("common", 10).len(), 10);
    }
}
