"""guard_gate.py —— 守卫判定金丝雀门（S195）。

**为什么再造一道门**（而非扩 claim_gate / H2）：
- `claim_gate` 管**文档数字 ↔ 真值源**；H2 管**文件分支一致率/漏判**（`kind==file`，
  评测自设沙盒 `'*'`）。守卫的**判定形状**——工具名误杀、扩展名静默消失、`:0` 假 verified、
  符号空转、沙盒钳制被拆——两边都不覆盖。P1-P6 那批缺陷全部属于这类形状回归。
- 判据与 claim_gate 同族：**逐项复算**。语料（`spec/guard-corpus.json`）里每条声明的
  kind/status/detail 关键词与「未核查」计数都必须与守卫当场输出逐位一致——
  **多判一条也红**（防"新增误杀"这种最阴的回归），少判一条也红（防"静默消失"复活）。
- 双模式即双口径：`open`（沙盒 `'*'`，评测态）与 `restricted`（沙盒限 fixture，生产
  fail-closed 态）分别钉住——**评测态 ≠ 部署态**从口头纪律变成会被判红的账。

fail-closed：语料缺失 / 坏 JSON / 空 modes / 条目缺字段 / fixture 缺失 ⇒ 红
（登记表被清空 = 门失效，与 claim_gate 同罪）。

用法：
  python -X utf8 scripts/guard_gate.py                              # 扫本仓
  python -X utf8 scripts/guard_gate.py --root <r> --corpus <json>   # 夹具/判据用
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = "spec/guard-corpus.json"
CASE_KEYS = ("id", "root", "text", "items", "unchecked_words", "unchecked_cites")
VALID_STATUS = ("verified", "refuted", "unverifiable")


def _tokens(root: pathlib.Path) -> dict[str, str]:
    return {"@REPO": root.as_posix(),
            "@FIXTURE": (root / "bench" / "fixtures" / "guard").as_posix()}


def _subst(s: str, tok: dict[str, str]) -> str:
    for k, v in tok.items():
        s = s.replace(k, v)
    return s


def _validate_case(case: dict[str, Any], loc: str, problems: list[str]) -> None:
    missing = [key for key in CASE_KEYS if key not in case]
    problems.extend(f"case[{loc}] 缺字段 {key}" for key in missing)
    bad = [item for item in case.get("items", [])
           if not {"decl", "kind", "status"} <= set(item)
           or item.get("status") not in VALID_STATUS]
    problems.extend(f"case[{loc}] item 形状非法: {item}" for item in bad)


def _load_corpus(path: pathlib.Path, problems: list[str]) -> dict[str, Any] | None:
    """fail-closed 的结构校验：缺失/坏 JSON/空 modes/条目缺字段/非法 status 都记进 problems。"""
    if not path.is_file():
        problems.append(f"语料缺失：{path}")
        return None
    try:
        corpus = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        problems.append(f"语料坏 JSON：{e}")
        return None
    modes = corpus.get("modes")
    if not isinstance(modes, list) or not modes:
        problems.append("语料 modes 为空或非列表（门被清空即失效）")
        return None
    for mi, mode in enumerate(modes):
        missing = [key for key in ("mode", "sandbox", "cases") if key not in mode]
        problems.extend(f"mode[{mi}] 缺字段 {key}" for key in missing)
        cases = mode.get("cases") or []
        if not cases:
            problems.append(f"mode[{mi}] cases 为空")
        for ci, case in enumerate(cases):
            _validate_case(case, f"{mi}.{ci}", problems)
    return corpus


def _diff_case(case: dict[str, Any], res: dict[str, Any], tok: dict[str, str],
               problems: list[str]) -> None:
    cid = _subst(str(case["id"]), tok)
    expected = {_subst(str(i["decl"]), tok) for i in case["items"]}
    got = {str(g["decl"]) for g in res.get("results", [])}
    if expected != got:
        problems.append(f"{cid}: 判定集不等 多出={sorted(got - expected)} "
                        f"缺失={sorted(expected - got)}")
    by_decl = {str(g["decl"]): g for g in res.get("results", [])}
    for item in case["items"]:
        decl = _subst(str(item["decl"]), tok)
        g = by_decl.get(decl)
        if g is None:
            continue
        if g.get("kind") != item["kind"]:
            problems.append(f"{cid}: {decl} kind={g.get('kind')} 期望 {item['kind']}")
        if g.get("status") != item["status"]:
            problems.append(f"{cid}: {decl} status={g.get('status')} 期望 {item['status']}"
                            f"（detail={g.get('detail', '')[:60]}）")
        want = item.get("detail_contains")
        if want and want not in str(g.get("detail", "")):
            problems.append(f"{cid}: {decl} detail 缺关键词「{want}」")
    unc = res.get("未核查", {})
    actual_words = sorted(unc.get("非声明小写词", {}).get("样例", []))
    actual_cites = sorted(unc.get("白名单外扩展名引用", {}).get("样例", []))
    for key, actual in (("unchecked_words", actual_words), ("unchecked_cites", actual_cites)):
        exp = sorted(_subst(str(x), tok) for x in case[key])
        if actual != exp:
            problems.append(f"{cid}: {key} 实际={actual} 期望={exp}")


def _run(corpus: dict[str, Any], root: pathlib.Path, tok: dict[str, str],
         problems: list[str]) -> int:
    sys.path.insert(0, str(root))
    import registry  # 运行期注入路径，门独立于包结构
    import tools  # noqa: F401  # 副作用注册，缺它 tool_count=0 全判假

    n_cases = 0
    for mode in corpus["modes"]:
        os.environ["UNIFIED_RX_SANDBOX"] = _subst(str(mode["sandbox"]), tok)
        for case in mode["cases"]:
            n_cases += 1
            out = registry.call_with_context(
                "hallucination_guard",
                {"text": _subst(str(case["text"]), tok),
                 "root": _subst(str(case["root"]), tok)},
                request_id="guard-gate")
            if not out.get("ok"):
                problems.append(f"{case['id']}: 守卫调用失败 {out.get('error')}")
                continue
            _diff_case(case, out["result"], tok, problems)
    return n_cases


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    ap.add_argument("--corpus", default=None)
    a = ap.parse_args(argv)
    root = pathlib.Path(a.root).resolve()
    cpath = pathlib.Path(a.corpus) if a.corpus else root / DEFAULT_CORPUS
    problems: list[str] = []
    corpus = _load_corpus(cpath, problems)
    tok = _tokens(root)
    if corpus is not None and not (root / "bench" / "fixtures" / "guard").is_dir():
        problems.append("fixture 目录缺失：bench/fixtures/guard")
    n_cases = 0
    n_modes = len(corpus["modes"]) if corpus else 0
    if corpus is not None and not problems:
        n_cases = _run(corpus, root, tok, problems)
    if problems:
        for p in problems:
            print(f"  ✗ {p}")
        print(f"GUARD-GATE FAIL 原因={len(problems)}")
        return 1
    print(f"GUARD-GATE OK cases={n_cases} modes={n_modes} 语料=guard-corpus.json")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
