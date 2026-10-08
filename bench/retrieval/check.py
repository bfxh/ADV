"""M4-1 的门入口：一次跑完三件事，任何一件不过就判红。

  ① **生成器可重放**：`gen_queries --check` 重跑一遍并与冻结的 `corpus.sha256` / `queries.jsonl` 逐字节对账
     （语料或扰动手法一改就会露）；
  ② **冻结查询仍过三条判据**：把 `queries.jsonl` 里的每条重新喂 `protocol.verdict_prepared`
     —— 光"生成时过门"不够，冻结件被手改过也要当场抓住；
  ③ **地板基线读数一致**：`floor --check`（含语料 sha256 漂移检查；漂移 ⇒ 判不了，不是判红）。

退出码：0 全绿 / 1 判红 / 3 判不了（语料漂移、冻结件缺失）。

用法：python -X utf8 bench/retrieval/check.py
"""
from __future__ import annotations

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import floor as floor_mod  # noqa: E402
import gen_queries  # noqa: E402
import protocol  # noqa: E402


def verify_frozen_queries(docs: list[tuple[str, str]], queries: list[dict]) -> list[str]:
    """冻结件里的每条查询重过三条判据；返回违规清单。"""
    prepared = protocol.prepare([t for _id, t in docs])
    tokens_by_id = dict(zip([d for d, _t in docs], prepared[1]))
    bad: list[str] = []
    for q in queries:
        source = q.get("source", "")
        if source not in tokens_by_id:
            bad.append(f"{q.get('qid')}：金标源 {source} 不在语料里")
            continue
        ok, why = protocol.verdict_prepared(q["text"], tokens_by_id[source], prepared)
        if not ok:
            bad.append(f"{q.get('qid')}：{why[0]}")
    return bad


def main() -> int:
    if gen_queries.main(["--check"]) != 0:
        return 1
    docs, drift = floor_mod.load_corpus()
    if drift:
        print(f"RETRIEVAL-CHECK INCONCLUSIVE：语料漂移 {len(drift)} 处：{drift[:5]}")
        return 3
    if not gen_queries.QUERIES.is_file():
        print("RETRIEVAL-CHECK INCONCLUSIVE：queries.jsonl 不在")
        return 3
    queries = [json.loads(l) for l in gen_queries.QUERIES.read_text(encoding="utf-8").splitlines() if l.strip()]
    bad = verify_frozen_queries(docs, queries)
    if bad:
        print(f"RETRIEVAL-CHECK FAIL：冻结查询有 {len(bad)} 条不过判据：{bad[:5]}")
        return 1
    rc = floor_mod.main(["--check"])
    if rc != 0:
        return rc
    print(f"RETRIEVAL-CHECK OK：语料 {len(docs)} 篇 / 查询 {len(queries)} 条 / 地板基线对账通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
