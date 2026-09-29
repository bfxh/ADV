"""hub_mutate.py —— 突变测试第一片（S189，spec/FRONTIER-CI.md §5.3 第 11 项，方向二十二）。

覆盖率只回答"代码被执行过"，本模块回答"测试抓不抓得住 bug"：对**变更行**生成三类变异
（算符 / 常量 / 边界），用 `hub_impact.select_for` **只跑受影响的测试**；被杀 = 选测红。
存活者再用**全量**复核（有界）：全量杀掉的记 `selection_missed`（选测漏了杀手测试——
"选测不丢"的诚实账），全量也杀不掉的才是**真存活**（测试缺口）。选测为空 = 没有测试
可达该变异，直接记待复核（不为它白跑一趟）。

**安全（本模块最重要的契约）**：变异 = 原地改写 + 落盘备份 + 终验恢复——
① 改前把原文件写到 `<file>.mutbak`；② 无论红绿/超时/异常**先恢复再校验**（sha256 对原始，
不一致立即抛——原文件可从 .mutbak 手工回写）；③ 启动即清扫上次运行残留的 `.mutbak`
（进程被 SIGKILL 的情形），恢复并如实计入 `degraded`。
**不要在有并发会话的工作树上跑**；判据/验收一律 `--root` 指向隔离副本（判据正是这么写的）。

CLI（JSON 一种形态）：
  python -X utf8 hub_mutate.py --root <dir> --base HEAD --max 24 [--no-full-verify]
"""
from __future__ import annotations

import ast
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

import hub_impact
import hub_propose

MUTBAK = ".mutbak"
_ARITH = {ast.Add: ast.Sub, ast.Sub: ast.Add, ast.Mult: ast.Div, ast.Div: ast.Mult}
_BOOL = {ast.And: ast.Or, ast.Or: ast.And}
_CMP_FLIP = {ast.Eq: ast.NotEq, ast.NotEq: ast.Eq}
_CMP_BOUNDARY = {ast.Lt: ast.LtE, ast.LtE: ast.Lt, ast.Gt: ast.GtE, ast.GtE: ast.Gt}
_HUNK_RE = re.compile(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")
_MAX_SURVIVOR_VERIFY = 6


def _changed_lines(root: pathlib.Path, rel: str, base: str) -> set[int] | None:
    """变更行号集（新文件侧）；None = 全文件（未跟踪——保守取全部，不猜 diff）。"""
    rc, _tracked, _err = hub_propose._git(root, ["ls-files", "--error-unmatch", rel])
    if rc != 0:
        return None
    rc, diff, _err = hub_propose._git(root, ["diff", "-U0", base, "--", rel])
    if rc != 0:
        return None
    lines: set[int] = set()
    for m in (_HUNK_RE.match(ln) for ln in diff.splitlines()):
        if m:
            start, count = int(m.group(1)), int(m.group(2) or 1)
            lines.update(range(start, start + count))
    return lines


def _gen_for_node(node: ast.expr) -> list[tuple[str, ast.AST]]:
    """单节点 → [(类, 替代节点)]；三类变异各归各类（判据按类点名）。全部是表达式节点。"""
    if isinstance(node, ast.BinOp) and type(node.op) in _ARITH:
        return [("算符", ast.BinOp(left=node.left, op=_ARITH[type(node.op)](), right=node.right))]
    if isinstance(node, ast.BoolOp) and type(node.op) in _BOOL:
        return [("算符", ast.BoolOp(op=_BOOL[type(node.op)](), values=node.values))]
    if isinstance(node, ast.Compare) and len(node.ops) == 1:
        t = type(node.ops[0])
        if t in _CMP_FLIP:
            return [("算符", ast.Compare(left=node.left, ops=[_CMP_FLIP[t]()],
                                         comparators=node.comparators))]
        if t in _CMP_BOUNDARY:
            return [("边界", ast.Compare(left=node.left, ops=[_CMP_BOUNDARY[t]()],
                                         comparators=node.comparators))]
    if isinstance(node, ast.Constant):
        if isinstance(node.value, bool):
            return [("常量", ast.Constant(value=not node.value))]
        if isinstance(node.value, (int, float)):
            return [("常量", ast.Constant(value=node.value + 1))]
    return []


def _mutants_of(src: str) -> list[dict]:
    """源码 → 变异候选（确定性顺序由调用方排序）；span = 精确替换段（跨行节点也准）。"""
    out: list[dict] = []
    try:
        tree = ast.parse(src)
    except SyntaxError:
        return []
    for node in ast.walk(tree):
        if not isinstance(node, ast.expr):
            continue                          # 三类变异只落在表达式上（位置属性也只在 expr）
        for kind, repl in _gen_for_node(node):
            new_seg = ast.unparse(repl)
            if "\n" in new_seg:
                continue                       # 只做单点变异（判据与"old→new"都要可读）
            out.append({"line": node.lineno, "col": node.col_offset, "kind": kind,
                        "old": ast.get_source_segment(src, node) or "",
                        "new": new_seg,
                        "span": (node.lineno, node.col_offset,
                                 node.end_lineno, node.end_col_offset)})
    return out


def _apply(src: str, span: tuple[int, int, int, int], new: str) -> str:
    """按 AST span 精确替换（同片段多次出现也不会错位——不依赖字符串查找）。"""
    l1, c1, l2, c2 = span
    lines = src.splitlines(keepends=True)
    head, tail = lines[l1 - 1][:c1], lines[l2 - 1][c2:]
    lines[l1 - 1:l2] = [head + new + (tail if tail.endswith("\n") else tail + "\n")]
    return "".join(lines)


def _cleanup_stale(root: pathlib.Path, degraded: list[str]) -> int:
    """清扫上次运行残留的 .mutbak（SIGKILL 情形）：有备份就恢复，计数可追。"""
    n = 0
    for bak in sorted(root.rglob(f"*{MUTBAK}")):
        orig = bak.with_name(bak.name[:-len(MUTBAK)])
        orig.write_bytes(bak.read_bytes())
        bak.unlink()
        degraded.append(f"恢复上次残留备份: {orig.relative_to(root).as_posix()}")
        n += 1
    return n


def _write_mutant(p: pathlib.Path, originals: dict[str, bytes], mut: dict) -> None:
    """改写前落 .mutbak（恢复的唯一样）；内容按变异后的源码写。"""
    (p.parent / (p.name + MUTBAK)).write_bytes(originals[mut["file"]])
    p.write_text(_apply(originals[mut["file"]].decode("utf-8", errors="replace"),
                        mut["span"], mut["new"]), encoding="utf-8")


def _restore(p: pathlib.Path, originals: dict[str, bytes], rel: str) -> None:
    """恢复 + sha256 终验：不一致立即抛（此刻原文件仍在 .mutbak 里，可手工回写）。"""
    p.write_bytes(originals[rel])
    bak = p.parent / (p.name + MUTBAK)
    if bak.exists():
        bak.unlink()
    if hashlib.sha256(p.read_bytes()).digest() != hashlib.sha256(originals[rel]).digest():
        raise RuntimeError(f"恢复校验失败: {rel}——原文件可从 .mutbak 回写")


def _run_tests(root: pathlib.Path, tests: list[str], timeout_s: int) -> tuple[bool, str]:
    """跑测试；返回 (是否被杀, 说明)。超时算被杀（死循环变异也是行为改变）。

    **必须清 `__pycache__` 且禁写 pyc**：pyc 失效校验是 mtime 整秒 + 大小——等长变异
    （`a * 2 → a / 2`）同秒写回时陈旧 pyc 会被当作有效 ⇒ 测试跑的是旧字节码
    （S189 实测：同一夹具三跑两样，被杀数 1/2/3 漂移——判据抓到的真缺陷）。
    """
    _clear_pycache(root)
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    argv = [sys.executable, "-B", "-m", "pytest", "-x", "-q", "-p", "no:cacheprovider",
            *(tests or ["tests/"])]
    try:
        cp = subprocess.run(argv, cwd=str(root), capture_output=True, timeout=timeout_s,
                            shell=False, env=env)
    except subprocess.TimeoutExpired:
        return True, "timeout"
    return cp.returncode != 0, f"exit={cp.returncode}"


def _clear_pycache(root: pathlib.Path) -> None:
    for d in sorted(root.rglob("__pycache__")):
        shutil.rmtree(d, ignore_errors=True)


def _select_for(root: pathlib.Path, rel: str) -> tuple[list[str] | None, str | None]:
    """任意文件 → 建议测试集（复用 hub_impact 的图与保守口径；与 impact 一字不差）。

    返回 (选测, 退化理由)；图不可用/解析失败率过高 ⇒ (None, 理由)——调用方按**空选测**
    如实记待复核并计 degraded，**不许**当作"没问题"。
    """
    forward, reverse, n_files, n_failed = hub_impact.build_graph(root)
    if n_files == 0:
        return None, "仓内无可解析的 .py"
    if n_failed and n_failed / n_files > hub_impact._PARSE_FAIL_TOLERANCE:
        return None, f"解析失败率 {n_failed}/{n_files} 过高——不可信就不选"
    affected = {rel} | hub_impact._closure([rel], reverse)
    selected, _unknown, _tests = hub_impact._selected_tests(forward, affected, "tests/")
    return selected, None


def _collect(root: pathlib.Path, base: str, originals: dict[str, bytes]) -> tuple[list, list]:
    """变更文件 × 变更行 → (changed, 候选变异，确定性排序)；tests/ 永不变异。"""
    changed = [f for f in hub_impact.changed_files(root, base) if not f.startswith("tests/")]
    cands: list[dict] = []
    for rel in sorted(changed):
        p = root / rel
        if not p.is_file():
            continue
        originals[rel] = p.read_bytes()
        lines = _changed_lines(root, rel, base)
        for mut in _mutants_of(originals[rel].decode("utf-8", errors="replace")):
            if lines is None or mut["line"] in lines:
                mut["file"] = rel
                cands.append(mut)
    cands.sort(key=lambda m: (m["file"], m["line"], m["col"], m["kind"], m["new"]))
    return changed, cands


def _run_mutants(root: pathlib.Path, cands: list[dict], originals: dict[str, bytes],
                 timeout_s: int, degraded: list[str]) -> tuple[list[dict], list[dict]]:
    """逐变异：改写 → 跑选测 → 恢复终验。返回 (被杀, 存活待复核)。"""
    sel_cache: dict[str, list[str] | None] = {}
    killed: list[dict] = []
    survived: list[dict] = []
    for mut in cands:
        rel = mut["file"]
        p = root / rel
        if rel not in sel_cache:
            sel_cache[rel], reason = _select_for(root, rel)
            if reason:
                degraded.append(f"{rel}: 选测退化（{reason}）——按空选测如实记待复核")
        _write_mutant(p, originals, mut)
        try:
            tests_to_run = sel_cache[rel] or []
            if tests_to_run:
                ok, note = _run_tests(root, tests_to_run, timeout_s)
                mut["_note"] = note
                (killed if ok else survived).append(mut)
            else:
                mut["_note"] = "选测为空——待全量复核"
                survived.append(mut)
        finally:
            _restore(p, originals, rel)
    return killed, survived


def _verify_full(root: pathlib.Path, survived: list[dict], originals: dict[str, bytes],
                 timeout_s: int) -> set[int]:
    """存活者全量复核（有界）：全量杀掉的 = 选测漏了杀手测试（selection_missed）。"""
    full_kills: set[int] = set()
    for mut in survived[:_MAX_SURVIVOR_VERIFY]:
        p = root / mut["file"]
        _write_mutant(p, originals, mut)
        try:
            ok, note = _run_tests(root, [], timeout_s)          # 全量（tests/ 目录）
            mut["_note"] = note
            if ok:
                full_kills.add(id(mut))
        finally:
            _restore(p, originals, mut["file"])
    return full_kills


def mutate(root: pathlib.Path, base: str = "HEAD", max_mutants: int = 24,
           timeout_s: int = 120, full_verify: bool = True) -> dict:
    """对变更行做突变测试 → 得分 + 存活者（全量复核后）+ 恢复对账。"""
    root = root.resolve()
    degraded: list[str] = []
    _cleanup_stale(root, degraded)
    originals: dict[str, bytes] = {}
    changed, cands = _collect(root, base, originals)
    capped_from = len(cands)
    cands = cands[:max(0, max_mutants)]
    kinds: dict[str, int] = {}
    for mut in cands:
        kinds[mut["kind"]] = kinds.get(mut["kind"], 0) + 1

    killed, survived = _run_mutants(root, cands, originals, timeout_s, degraded)
    full_kills = _verify_full(root, survived, originals, timeout_s) if full_verify else set()
    true_survivors = [m for m in survived if id(m) not in full_kills]
    selection_missed = [m for m in survived if id(m) in full_kills]
    total = len(cands)
    return {
        "ok": True, "root": str(root), "base": base,
        "total": total, "killed": len(killed) + len(selection_missed),
        "survived": len(true_survivors), "selection_missed": len(selection_missed),
        "score": round((len(killed) + len(selection_missed)) / total, 4) if total else None,
        "capped_from": capped_from, "kinds_generated": kinds,
        "changed_files": changed,
        "survivors": [{"file": m["file"], "line": m["line"], "kind": m["kind"],
                       "old": m["old"], "new": m["new"]} for m in true_survivors],
        "selection_missed_list": [{"file": m["file"], "line": m["line"], "kind": m["kind"]}
                                  for m in selection_missed],
        "degraded": degraded,
    }


def main(argv: list[str]) -> int:
    def opt(flag: str, default: str) -> str:
        return argv[argv.index(flag) + 1] if flag in argv else default

    res = mutate(pathlib.Path(opt("--root", str(hub_impact.repo_root()))).resolve(),
                 base=opt("--base", "HEAD"), max_mutants=int(opt("--max", "24")),
                 full_verify="--no-full-verify" not in argv)
    print(json.dumps(res, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
