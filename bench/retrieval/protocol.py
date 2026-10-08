"""检索评测的**配对协议**（M4-1）：防泄漏三条判据 + 统一的切词口径。

为什么单独成模块：机械生成的查询天生有泄漏风险——查询是从文档里长出来的，稍不注意就把答案
逐字带回去了。所以"生成"与"判泄漏"必须分开写：生成器只负责产出候选，**这里的三条判据才是准绳**
（生成器必须逐条过门槛，过不了就作废并计数）。判据本身是纯函数，金丝雀直接喂合成语料。

三条判据（任一不过 ⇒ 该查询作废）：

  C1 连续子串：`normalize(查询)` 不得作为子串出现在**任何**文档的 `normalize(文本)` 里
     （大小写折叠 + 空白折叠；按字符比，专抓"原样搬回去"）；
  C2 最长公共词段：查询与**源文档**的最长公共**连续词段** ≤ MAX_TOKEN_RUN（防"打乱词序但保留长片段"）；
  C3 词集 Jaccard：查询与**任何**文档的 token 集合 Jaccard < MAX_JACCARD（防近似复制/整句搬运）。

阈值是口径不是真理：改动它们等于改评测集，必须**显式重录**（`gen_queries.py --write`），
不能悄悄放宽——`tests/test_retrieval_protocol.py` 里有一条把三个阈值钉住的金丝雀。
"""
from __future__ import annotations

import re

#: C2 的最长公共连续词段上限（token 数）。
MAX_TOKEN_RUN = 6
#: C3 的 Jaccard 上限（严格小于）。
MAX_JACCARD = 0.60

_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_WORD_SPLIT = re.compile(r"[^A-Za-z0-9]+")


def normalize(text: str) -> str:
    """大小写折叠 + 空白折叠（C1 用）。"""
    return re.sub(r"\s+", " ", text).strip().lower()


def split_identifier(ident: str) -> list[str]:
    """标识符拆词：`loadWorkspace` → [load, workspace]；`load_workspace` 同理。

    规则：先按非字母数字切，再在每个片段内按 camelCase/连续大写切；全部小写化。
    """
    out: list[str] = []
    for chunk in _WORD_SPLIT.split(ident):
        if not chunk:
            continue
        parts = re.findall(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+|\d+", chunk)
        out.extend(p.lower() for p in parts if p)
    return out


def tokenize(text: str) -> list[str]:
    """文档/查询的统一切词口径（标识符按 `split_identifier` 拆，其余按词切）。"""
    out: list[str] = []
    for ident in _IDENT.findall(text):
        out.extend(split_identifier(ident))
    return out


def longest_common_token_run(a: list[str], b: list[str]) -> int:
    """最长公共**连续**词段长度（DP；两序列都短，不做优化）。"""
    if not a or not b:
        return 0
    prev = [0] * (len(b) + 1)
    best = 0
    for token_a in a:
        cur = [0] * (len(b) + 1)
        for j, token_b in enumerate(b, start=1):
            if token_a == token_b:
                cur[j] = prev[j - 1] + 1
                best = max(best, cur[j])
        prev = cur
    return best


def jaccard(a: list[str], b: list[str]) -> float:
    """词集 Jaccard（两集都空 ⇒ 0.0，不判为泄漏）。"""
    sa, sb = set(a), set(b)
    if not sa or not sb:
        return 0.0
    return len(sa & sb) / len(sa | sb)


def prepare(texts: list[str]) -> tuple[list[str], list[list[str]], list[set[str]]]:
    """语料侧的**一次性预计算**（归一化文本 / 词序列 / 词集合）。

    为什么要有它：候选 × 文档是乘积级——2443 × 2404 次重切词实测把生成拖到分钟级
    （第一次跑直接超时）。判据本身不变，只是把"每对都重算"改成"语料只算一次"。
    """
    tokens = [tokenize(t) for t in texts]
    return [normalize(t) for t in texts], tokens, [set(t) for t in tokens]


def verdict_prepared(
    query: str,
    source_tokens: list[str],
    prepared: tuple[list[str], list[list[str]], list[set[str]]],
) -> tuple[bool, list[str]]:
    """三条判据一起过（走预计算通道）。返回 (是否通过, 违规项清单)。"""
    norm_texts, _corpus_tokens, corpus_sets = prepared
    qt = tokenize(query)
    q = normalize(query)
    violations: list[str] = []
    if not q:
        return False, ["C0 空查询"]
    for i, text in enumerate(norm_texts):
        if q in text:
            violations.append(f"C1 查询连续子串出现在文档 #{i}")
            break
    run = longest_common_token_run(qt, source_tokens)
    if run > MAX_TOKEN_RUN:
        violations.append(f"C2 与源文档最长公共词段 {run} > {MAX_TOKEN_RUN}")
    qset = set(qt)
    for i, dset in enumerate(corpus_sets):
        if not qset or not dset:
            continue
        j = len(qset & dset) / len(qset | dset)
        if j >= MAX_JACCARD:
            violations.append(f"C3 与文档 #{i} 的 Jaccard {j:.2f} >= {MAX_JACCARD}")
            break
    return (not violations), violations


def verdict(query: str, source_text: str, corpus_texts: list[str]) -> tuple[bool, list[str]]:
    """单条版（金丝雀与小语料用）：内部建一次性预计算，语义与 `verdict_prepared` 逐字一致。"""
    return verdict_prepared(query, tokenize(source_text), prepare(corpus_texts))
