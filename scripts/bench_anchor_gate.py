"""bench_anchor_gate.py —— 测量锚可用性门（S198）。

**为什么**：H1/H2/H3 的真值树（VF3 等）随盘迁移消失后，bench 脚本仍会"跑成功"——
truth 检查全 False、产出一版格式正常但无意义的数字（量空气）。账面「漏判 0」这类
历史数字与「当前可复算」是两种账，此前没人钉这个区分。

判据（登记表 `spec/bench-anchors.json`，真值=ast 扫 `bench/*.py` 的字符串常量）：
  · 未登记的仓外盘符路径引用 ⇒ 红（新锚必须先入账——主判据，防增量）；
  · state=present ⇒ 目录必须存在；
  · state=detached ⇒ 每个 consumer 必须存在且经 `require_anchor` 走 ANCHOR-MISSING
    出口（缺锚即退出码 2，禁止静默）；且 consumer 确实引用了该锚（名实相符）；
  · state=legacy ⇒ consumer（退役脚手架）文件仍在即可，不再要求可跑。

fail-closed：登记表缺失/坏 JSON/空 anchors/条目缺字段/非法 state ⇒ 红。
真路径：门只读源码与文件系统，不 import 被测脚本。

用法：
  python -X utf8 scripts/bench_anchor_gate.py
  python -X utf8 scripts/bench_anchor_gate.py --root <dir> --anchors <json>   # 夹具/判据用
"""
from __future__ import annotations

import argparse
import ast
import json
import pathlib
import re
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
DRIVE_RE = re.compile(r"^[A-Za-z]:[\\/]")
STATES = ("present", "detached", "legacy")


def _norm(s: str) -> str:
    return re.sub(r"[\\/]+", "/", s.replace("\\\\", "/"))


def _match(anchor_path: str, const: str) -> bool:
    a, c = _norm(anchor_path), _norm(const)
    return c == a or c.startswith(a.rstrip("/") + "/")


def _file_path_consts(path: pathlib.Path) -> list[str]:
    """扫一个 py 文件里的盘符路径字符串常量（排除各级 docstring）。"""
    tree = ast.parse(path.read_text(encoding="utf-8"))
    skip: set[int] = set()
    holders = (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)
    for n in ast.walk(tree):
        if isinstance(n, holders) and n.body:
            first = n.body[0]
            if (isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant)
                    and isinstance(first.value.value, str)):
                skip.add(id(first.value))
    return [n.value for n in ast.walk(tree)
            if (isinstance(n, ast.Constant) and isinstance(n.value, str)
                and id(n) not in skip and DRIVE_RE.match(n.value))]


def _collect_refs(root: pathlib.Path, problems: list[str]) -> dict[str, list[str]]:
    refs: dict[str, list[str]] = {}
    for p in sorted((root / "bench").glob("*.py")):
        try:
            consts = _file_path_consts(p)
        except (SyntaxError, ValueError) as e:
            problems.append(f"bench/{p.name} 解析失败：{e}")
            continue
        if consts:
            refs[f"bench/{p.name}"] = consts
    return refs


def _validate_anchor(a: dict[str, Any], i: int, problems: list[str]) -> None:
    missing = [key for key in ("id", "path", "state", "consumers") if key not in a]
    problems.extend(f"anchor[{i}] 缺字段 {key}" for key in missing)
    if a.get("state") not in STATES:
        problems.append(f"anchor[{i}] 非法 state: {a.get('state')}")
    if a.get("state") in ("detached", "legacy") and not str(a.get("reason", "")).strip():
        problems.append(f"anchor[{i}] detached/legacy 必须写 reason")


def _load_table(path: pathlib.Path, problems: list[str]) -> tuple[list[dict[str, Any]], list[str]]:
    if not path.is_file():
        problems.append(f"锚登记表缺失：{path}")
        return [], []
    try:
        data: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        problems.append(f"锚登记表坏 JSON：{e}")
        return [], []
    anchors = data.get("anchors")
    if not isinstance(anchors, list) or not anchors:
        problems.append("锚登记表 anchors 为空（门被清空即失效）")
        return [], []
    for i, a in enumerate(anchors):
        _validate_anchor(a, i, problems)
    bad_locals = [lp for lp in data.get("local_paths", [])
                  if not lp.get("prefix") or not str(lp.get("why", "")).strip()
                  or str(lp.get("why", "")).startswith("TODO")]
    problems.extend(f"local_path[{i}] 非法（prefix/why 必填，占位不放行）: {lp}"
                    for i, lp in enumerate(bad_locals))
    locals_ = [str(lp["prefix"]) for lp in data.get("local_paths", [])
               if lp not in bad_locals]
    return anchors, locals_


def _check_anchors(root: pathlib.Path, anchors: list[dict[str, Any]],
                   refs: dict[str, list[str]], problems: list[str]) -> None:
    root_posix = _norm(str(root))
    for a in anchors:
        if a["state"] == "present" and not (pathlib.Path(a["path"])).is_dir():
            problems.append(f"[{a['id']}] present 但目录不存在：{a['path']}")
        consumers = [str(c) for c in a["consumers"]]
        for c in consumers:
            f = root / c
            if not f.is_file():
                problems.append(f"[{a['id']}] consumer 不存在：{c}")
                continue
            consts = refs.get(c, [])
            if not any(_match(a["path"], x) and not _match(root_posix, x) for x in consts):
                problems.append(f"[{a['id']}] consumer 名实不符（未引用该锚）：{c}")
            if a["state"] == "detached" and "require_anchor" not in f.read_text(encoding="utf-8"):
                problems.append(f"[{a['id']}] detached 锚的 consumer 缺 require_anchor 出口：{c}")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    ap.add_argument("--anchors", default=None)
    a = ap.parse_args(argv)
    root = pathlib.Path(a.root).resolve()
    apath = pathlib.Path(a.anchors) if a.anchors else root / "spec" / "bench-anchors.json"
    problems: list[str] = []
    anchors, locals_ = _load_table(apath, problems)
    refs = _collect_refs(root, problems)
    root_posix = _norm(str(root))
    registered = {str(x["path"]) for x in anchors} | {_norm(p) for p in locals_}
    for fname, consts in refs.items():
        for c in consts:
            if _match(root_posix, c):
                continue  # 指向本仓自身，不算外部锚
            if not any(_match(ap_, c) for ap_ in registered):
                problems.append(f"未登记的外部路径引用：{fname} -> {c}")
    if anchors:
        _check_anchors(root, anchors, refs, problems)
    if problems:
        for p in problems:
            print(f"  ✗ {p}")
        print(f"BENCH-ANCHOR-GATE FAIL 原因={len(problems)}")
        return 1
    n_refs = sum(len(v) for v in refs.values())
    states = "/".join(str(x["state"]) for x in anchors)
    print(f"BENCH-ANCHOR-GATE OK anchors={len(anchors)} refs={n_refs} states={states}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
