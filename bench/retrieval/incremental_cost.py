"""M4-4b 的**成本读数**（不是门）：同一批钉版语料上，增量复用比全量重建省下多少。

驱动 `target/debug/incremental_cost.exe`（四段各 3 次取中位：全量重建 / 冷 apply /
热 apply（全复用）/ 复用后重建倒排），并自己做两条独立核对：

  ① **token 账**：冷+热两轮之后 `tokens_tokenized` 必须**正好等于语料的 token 总数**
     （= 热轮一次都没重切词）。这条由 python 侧用 `protocol.tokenize` 自己数出来对，
     不看 Rust 的计数逻辑；
  ② **正确性自检**：exe 自己比较"复用产物 vs 全量重建"，不等就非 0 退出。

**为什么不是门**：墙钟随机器负载漂，本仓的计时档要独占（见 `cli-bench`/`perf-gate` 的
SKIP 口径），这里不设阈值、不冻结数字，只出读数。账本引用它时要带上跑出当天的负载条件。

退出码：0 读数成立 / 1 核对不上（token 账或 parity 任一不过）/ 3 判不了（exe 不在、语料漂移）。

用法：python -X utf8 bench/retrieval/incremental_cost.py
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import floor  # noqa: E402  —— 语料与 sha256 核对复用地板那把尺
import protocol  # noqa: E402

EXE = ROOT / "target" / "debug" / "incremental_cost.exe"


def payload(docs: list[tuple[str, str]]) -> bytes:
    lines = [
        json.dumps({"type": "doc", "doc_id": doc_id, "text": text},
                   ensure_ascii=False, separators=(",", ":"))
        for doc_id, text in docs
    ]
    return ("\n".join(lines) + "\n").encode("utf-8")


def main() -> int:
    if not EXE.is_file():
        print(f"INCREMENTAL-COST INCONCLUSIVE：CLI 不在（先 cargo build -p adv-index --bin incremental_cost）：{EXE}")
        return 3
    docs, drift = floor.load_corpus()
    if drift:
        print(f"INCREMENTAL-COST INCONCLUSIVE：语料漂移 {len(drift)} 处（需显式重录）：{drift[:5]}")
        return 3

    proc = subprocess.run([str(EXE)], input=payload(docs),
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip() or proc.stdout.decode("utf-8", "replace").strip()
        print(f"INCREMENTAL-COST FAIL：exe 退出码 {proc.returncode}（parity 自检不过也算这里）：{detail}")
        return 1

    got = json.loads(proc.stdout.decode("utf-8"))
    ms = got["median_ms"]
    expected_tokens = sum(len(protocol.tokenize(text)) for _id, text in docs)
    print(json.dumps(got, ensure_ascii=False, indent=2))
    print(f"语料 token 总数（python 侧自己数）= {expected_tokens}")

    bad: list[str] = []
    if got["docs"] != len(docs):
        bad.append(f"篇数对不上：exe={got['docs']} python={len(docs)}")
    if got["tokens_tokenized_after_two_rounds"] != expected_tokens:
        bad.append(
            "热轮没有做到零重切词："
            f"exe 两轮累计 {got['tokens_tokenized_after_two_rounds']} vs 语料 {expected_tokens}"
            "（两轮都切的话应约为 2×）"
        )
    if got["tokens_corpus"] != expected_tokens:
        bad.append(
            f"Rust 侧自己数的语料 token 与 python 侧不等：exe={got['tokens_corpus']} python={expected_tokens}"
            "（切词口径本该由 `tokenizer_protocol.rs` 钉住，这里再对一次）"
        )
    if not got["parity_with_full_build"]:
        bad.append("exe 自检：复用产物与全量重建不等")
    if bad:
        print("INCREMENTAL-COST FAIL：\n  " + "\n  ".join(bad))
        return 1

    saved = ms["full_build"] - ms["warm_apply"] - ms["index_rebuild"]
    print(
        "INCREMENTAL-COST OK：全量 "
        f"{ms['full_build']:.1f}ms → 热复用 {ms['warm_apply']:.1f}ms + 重建倒排 {ms['index_rebuild']:.1f}ms"
        f"（省下约 {saved:.1f}ms / {saved / max(ms['full_build'], 1e-9) * 100:.0f}%）"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
