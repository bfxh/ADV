"""h2_guard_eval.py —— H2 首测：hallucination_guard 判定 vs 路径存在性真值的一致率。

数据源：bench/results/l3/** 已收集的答案（本会话双臂实验产物，零额外 API 成本）。
真值口径：与 ab_run.halluc_rate 同一文件存在性检查（VF3_ROOT 下 isfile）。
一致性：guard=refuted ↔ 文件不存在；其余(verified/unverifiable) ↔ 文件存在；
        guard 行号语义更细（存在但行号越界仍判 refuted），此类真值侧计"宽"，单列不扣分。

用法：python bench/h2_guard_eval.py [--arm bare_model]   # 默认双臂都测
"""
import argparse
import glob
import json
import os
import pathlib
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)

# S97：bench 显式声明沙盒（与 s94_perf.py 同纪律）——guard 已过沙盒门
# （S88 补漏），裸 shell 下 fail-closed 会把真值判成"不可验证"干扰测量。
os.environ.setdefault("UNIFIED_RX_SANDBOX", "*")


from anchor_guard import (  # noqa: E402  # S198：入口校验锚在场（模块级禁用 require——tests/swe 会 import 本模块）
    env_anchor,
    require_anchor,
)

import registry  # noqa: E402
import tools  # noqa: F401,E402
from tools.guard import _CLAIM_EXTS  # noqa: E402  与守卫同一张白名单，防口径漂移

VF3_ROOT = env_anchor("UNIFIED_RX_ANCHOR_VF3", r"D:\开发\VoxelForge-V3")
FILE_RE = re.compile(r"([A-Za-z0-9_./\\\-]+\.(?:"
                     + "|".join(sorted(_CLAIM_EXTS, key=len, reverse=True)) + r"))"
                     r"(?::([A-Za-z0-9_]+))?")


def truth_exists(path_claim):
    p = path_claim.replace("\\", "/").lstrip("./")
    return os.path.isfile(os.path.join(VF3_ROOT, p))


def evaluate(answers):
    agree = strict_agree = total = 0
    refuted_n = refuted_true = 0
    rows = []
    for a in answers:
        r = registry.call_with_context("hallucination_guard", {"text": a, "root": VF3_ROOT},
                                       request_id="h2-eval")
        if not r.get("ok"):
            continue
        for item in r["result"]["results"]:
            if item["kind"] != "file":
                continue
            m = FILE_RE.fullmatch(item["decl"].strip("`"))
            if not m:
                continue
            claim, suffix = m.group(1), m.group(2)
            exists = truth_exists(claim)
            total += 1
            if item["status"] == "refuted":
                refuted_n += 1
                refuted_true += not exists
            # 宽口径一致：refuted↔不存在；verified/unverifiable↔存在
            ok_wide = (item["status"] == "refuted") == (not exists)
            # 严口径：宽一致且不存在时不依赖行号辩解 / 存在时不得因 suffix 被拒
            ok_strict = ok_wide and (
                exists or (item["status"] == "refuted"
                           and ("不存在" in item["detail"] or not suffix)))
            agree += ok_wide
            strict_agree += ok_strict
            if len(rows) < 400:
                rows.append({"claim": item["decl"], "exists": exists,
                             "guard": item["status"], "wide_ok": ok_wide})
    return {"claims": total,
            "wide_agreement": round(agree / total, 4) if total else None,
            "strict_agreement": round(strict_agree / total, 4) if total else None,
            # P5 补口径：证伪精确率——refuted 里真不存在的占比（误杀会在这里现形；
            # 旧口径只测一致率与漏判，工具名分支的假阳性完全不在测量面内）
            "refuted": refuted_n,
            "refuted_precision": round(refuted_true / refuted_n, 4) if refuted_n else None,
            "samples": rows[:200]}


def main():
    require_anchor("vf3", VF3_ROOT)
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", default=None, help="bare_model/model_plus_rx，默认全部")
    args = ap.parse_args()
    report = {"root": VF3_ROOT,
              # P5：口径如实入档——本评测在何种沙盒下跑的（受限沙盒时大量声明
              # 会落 unverifiable，一致率含义完全不同），以及范围注记。
              "sandbox": os.environ.get("UNIFIED_RX_SANDBOX"),
              "scope_note": "仅 kind=file；工具/符号分支不在 H2 口径内",
              "arms": {}}
    pattern = os.path.join(HERE, "results", "l3", "*", "*", "*.json")
    by_arm = {}
    for fp in sorted(glob.glob(pattern)):
        d = json.loads(pathlib.Path(fp).read_text(encoding="utf-8"))
        arm = fp.replace("\\", "/").split("/l3/")[1].split("/")[0]
        if args.arm and arm != args.arm:
            continue
        ans = d.get("answer")
        if ans:
            by_arm.setdefault(arm, []).append(ans)
    for arm, answers in sorted(by_arm.items()):
        rep = evaluate(answers)
        report["arms"][arm] = rep
        print(f"{arm:16s} claims={rep['claims']:4d} "
              f"wide={rep['wide_agreement']} strict={rep['strict_agreement']} "
              f"refuted={rep['refuted']} refuted_precision={rep['refuted_precision']}")
    out = os.path.join(HERE, "results", "l3", "h2_report.json")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    pathlib.Path(out).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"[OK] 写入 {os.path.relpath(out, HERE)}")


if __name__ == "__main__":
    main()
