"""设计债到期门（docs/DESIGN-DEBT-GATE.md）：让"设计上的问题"必须显式、且会过期。

为什么要有它：仓里有 26 道门管"维护形"的债（规模棘轮、重复、命名、基线只准减），
但没有一道门管**设计形**的缺口——观察面为空、判据只撑单侧、能力缺失。这类东西
不会因为"这次改动没碰它"而变红，于是可以无限期沉默。用户 2026-10-06 定的硬约束是：
**登记的债最迟在下 10 个任务内处置，超期由门自己想起来判红。**

判据（全部机器可判；命中即红）：
  D1 必填字段齐（`required_fields`）；
  D2 `why_from_design` 非空——写不出设计成因的条目就是"贴标签躲门"；
  D3 `1 ≤ due_after_tasks ≤ 10`（上限不许自行放宽）；
  D4 `since` / `origin` 必须是可解析的 commit（防"凭印象写一个 SHA"：
     本门作者 2026-10-06 就连着写错过两次，这条判据是那次教训的固化）；
  D5 超期即红：`git rev-list --count <since>..HEAD > due_after_tasks`；
  D6 id 唯一，且不许同时出现在册与已退役两处；
  D7 已退役条目必须带非空 `evidence`——销账要有交代，不许一删了之。

计时口径：`since` = 登记那次的 commit（既不往前倒推里程，也不许重开刷新计时）；
`origin` = 设计成因所在的 commit（只追溯、不计时）。

用法：python scripts/debt_gate.py   （退出码 0 = 通过；1 = 命中）
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEBT = ROOT / "spec" / "design-debt.json"
MATURITY = ROOT / "spec" / "maturity.json"
MAX_DUE = 10
SHA_RE = re.compile(r"^[0-9a-f]{7,40}$")


def rev_count_since(root: Path, since: str) -> int:
    """`git rev-list --count <since>..HEAD`——"任务"的计数单位（2026-10-06 选定 commit 数）。"""
    out = subprocess.run(
        ["git", "rev-list", "--count", f"{since}..HEAD"],
        cwd=str(root),
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip() or f"git rev-list 失败：{since}")
    return int(out.stdout.strip())


def commit_exists(root: Path, sha: str) -> bool:
    return (
        subprocess.run(
            ["git", "cat-file", "-e", f"{sha}^{{commit}}"],
            cwd=str(root),
            capture_output=True,
        ).returncode
        == 0
    )


def _sha_fails(eid, field, sha, exists):
    """D4：since/origin 必须是**在 git 里查得到**的 commit 形态。"""
    if not isinstance(sha, str) or not sha:
        return [f"D4 {eid} 的 {field} 不是非空字符串：{sha!r}"]
    if not SHA_RE.match(sha):
        return [f"D4 {eid} 的 {field}={sha!r} 不像 commit 号"]
    if not exists(sha):
        return [f"D4 {eid} 的 {field}={sha} 在 git 里不存在（不许凭印象写 SHA）"]
    return []


def _due_fails(eid, entry, due, counter, exists):
    """D5：超期判定。计数真的来自 `git rev-list`（测试用注入的 counter 验这一点）。"""
    since = entry.get("since")
    if not (
        isinstance(since, str) and SHA_RE.match(since) and isinstance(due, int) and exists(since)
    ):
        return []
    walked = counter(since)
    if walked <= due:
        return []
    return [f"D5 {eid} 已拖过 {walked} 个 commit（限期 {due}）：{entry.get('defect', '')[:60]}"]


def _check_one(entry, required, *, counter, exists, seq):
    """一条在册债的 D1–D5（D6 查重在调用方）。"""
    eid = entry.get("id") or f"<无 id #{seq}>"
    fails = [f"D1 {eid} 缺必填字段：{f}" for f in required if f not in entry]
    if not (entry.get("why_from_design") or "").strip():
        fails.append(f"D2 {eid} 没写设计成因（why_from_design 为空）——没成因的登记等于贴标签躲门")
    due = entry.get("due_after_tasks")
    if isinstance(due, int) and not (1 <= due <= MAX_DUE):
        fails.append(
            f"D3 {eid} due_after_tasks={due} 越界（上限 {MAX_DUE}，要更长须升级为承重面讨论）"
        )
    fails += [
        f for field in ("since", "origin") for f in _sha_fails(eid, field, entry.get(field), exists)
    ]
    return fails + _due_fails(eid, entry, due, counter, exists)


def check_entries(entries, retired, *, required, counter, exists):
    """纯判据：喂进 registry 与两个注入点（计数器 / SHA 可核），返回违规文案列表。

    不读文件、不调 git ⇒ 每条判据都有对应的反向用例（少一个字段、超期、编造的 SHA
    各自要能单独红）。
    """
    fails = []
    seen = set()
    for seq, e in enumerate(entries, 1):
        eid = e.get("id") or f"<无 id #{seq}>"
        if eid in seen:
            fails.append(f"D6 {eid} id 重复")
        seen.add(eid)
        fails += _check_one(e, required, counter=counter, exists=exists, seq=seq)
    for e in retired:
        eid = e.get("id") or "<无 id>"
        if eid in seen:
            fails.append(f"D6 {eid} 同时在册与已退役")
        seen.add(eid)
        if not (e.get("evidence") or "").strip():
            fails.append(f"D7 {eid} 退役却无 evidence（销账必须有交代，不许一删了之）")
    return fails


REQUIRED = [
    "id",
    "surface",
    "kind",
    "defect",
    "why_from_design",
    "origin",
    "since",
    "due_after_tasks",
    "guards",
]

MATURITY_REQUIRED = ["pattern", "why", "guards"]


def check_maturity(entries):
    """试验面登记：必填之外，pattern 不许是"整个仓库"这种一放全放的形态。"""
    fails = []
    seen = set()
    for e in entries:
        pat = e.get("pattern") or f"<无 pattern #{len(seen) + 1}>"
        if pat in seen:
            fails.append(f"试验面 pattern 重复：{pat}")
        seen.add(pat)
        for f in MATURITY_REQUIRED:
            if f not in e:
                fails.append(f"试验面 {pat} 缺必填字段：{f}")
            elif f != "pattern" and not str(e.get(f) or "").strip():
                # 空 why / 空 guards 与缺字段同罪：登记的作用就是说清"谁读它、变了谁会红"
                fails.append(f"试验面 {pat} 的 {f} 为空")
        if isinstance(pat, str) and (pat in ("**", "*", "") or pat.startswith("**")):
            fails.append(f"试验面 pattern {pat!r} 过宽（会把承重面一起放掉）")
    return fails


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    if not DEBT.is_file():
        print(f"DEBT-GATE FAIL: 缺 {DEBT.relative_to(ROOT)}")
        return 1
    debt = load(DEBT)
    fails = check_entries(
        debt.get("entries", []),
        debt.get("retired", []),
        required=debt.get("required_fields") or REQUIRED,
        counter=lambda s: rev_count_since(ROOT, s),
        exists=lambda s: commit_exists(ROOT, s),
    )
    if MATURITY.is_file():
        fails += check_maturity(load(MATURITY).get("entries", []))
    else:
        fails.append(f"缺 {MATURITY.relative_to(ROOT)}（试验面登记）")

    live = debt.get("entries", [])
    if fails:
        for f in fails[:20]:
            print(f"  ❌ {f}")
        print(f"DEBT-GATE FAIL: {len(fails)} 处（规则见 docs/DESIGN-DEBT-GATE.md）")
        return 1
    aged = ", ".join(
        f"{e['id']} {rev_count_since(ROOT, e['since'])}/{e['due_after_tasks']}" for e in live
    )
    print(f"DEBT-GATE OK（在册 {len(live)} 条，已退役 {len(debt.get('retired', []))} 条）：{aged}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
