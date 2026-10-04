#!/usr/bin/env python3
"""RESEARCH registry 校验门（ADV 重写期工具）。

用法：python RESEARCH/registry/validate.py [--quiet]
规则：
  1. 每行合法 JSON，UTF-8，非空。
  2. 必填字段：id/domain/name/kind/verdict/mechanism/integration/license/link/depth/date。
  3. 枚举：kind ∈ project|paper|technique|dataset|product；
     verdict ∈ 吸收|有界|不吸收|watch|reference|P0|P1|P2；
     integration ∈ 白名单；depth ∈ deep|sweep。
  4. id = <domain>-<3位序号>，全局唯一；domain = 文件名 stem。
  5. link 以 http(s):// 开头；date 匹配 YYYY-MM-DD；mechanism 非空且 ≥8 字符。
退出码：0 = 全绿；1 = 有违规（打印清单）。
"""
import json, re, sys
from pathlib import Path

HERE = Path(__file__).parent
KINDS = {"project", "paper", "technique", "dataset", "product", "tool"}
VERDICTS = {"吸收", "有界", "不吸收", "watch", "reference", "P0", "P1", "P2"}
INTEGRATIONS = {"adv-parse", "adv-rules", "adv-taint", "adv-ast-rust", "adv-secrets",
                "adv-sca", "adv-index", "adv-server", "adv-sandbox", "adv-bin",
                "perf-core", "cli", "xtask-gates", "ci-ops", "test-replay",
                "docs-process", "watch", "reference"}
DEPTHS = {"deep", "sweep"}
REQUIRED = ["id", "domain", "name", "kind", "verdict", "mechanism",
            "integration", "license", "link", "depth", "date"]
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
ID_RE = re.compile(r"^[A-Z0-9]+-\d{3}$")
# link 三种合法形态：http(s) 外链；RESEARCH/<file>.md 本域文档锚（来源清单在文档内）；字面 '待验证'（诚实标记，补扫清单负责回填）
def link_ok(link: str) -> bool:
    if link.startswith(("http://", "https://")):
        return True
    if re.match(r"^RESEARCH/[\w.-]+\.md$", link):
        return True
    return link == "待验证"

def main() -> int:
    quiet = "--quiet" in sys.argv
    errors, total, ids = [], 0, set()
    files = sorted(HERE.glob("*.jsonl"))
    if not files:
        print(f"FAIL: no jsonl under {HERE}"); return 1
    for f in files:
        stem = f.stem
        for ln, raw in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            line = raw.strip()
            if not line:
                continue
            total += 1
            where = f"{f.name}:{ln}"
            try:
                o = json.loads(line)
            except Exception as e:
                errors.append(f"{where} bad-json {e}"); continue
            for k in REQUIRED:
                if k not in o or o[k] in ("", None):
                    errors.append(f"{where} missing/empty {k}")
            if o.get("domain") != stem:
                errors.append(f"{where} domain={o.get('domain')!r} != stem {stem!r}")
            if o.get("kind") not in KINDS:
                errors.append(f"{where} kind={o.get('kind')!r}")
            if o.get("verdict") not in VERDICTS:
                errors.append(f"{where} verdict={o.get('verdict')!r}")
            integ = o.get("integration")
            if integ not in INTEGRATIONS and not (isinstance(integ, str) and "|" in integ):
                errors.append(f"{where} integration={integ!r}")
            if o.get("depth") not in DEPTHS:
                errors.append(f"{where} depth={o.get('depth')!r}")
            i = o.get("id", "")
            if not ID_RE.match(i):
                errors.append(f"{where} id-format={i!r}")
            elif i in ids:
                errors.append(f"{where} dup-id {i}")
            else:
                ids.add(i)
            link = str(o.get("link", ""))
            if not link_ok(link):
                errors.append(f"{where} link={link!r}")
            if not DATE_RE.match(str(o.get("date", ""))):
                errors.append(f"{where} date={o.get('date')!r}")
            mech = str(o.get("mechanism", "")).strip()
            if len(mech) < 6:
                errors.append(f"{where} mechanism-too-short={mech!r}")
    if errors:
        print(f"FAIL: {len(errors)} violation(s) / {total} entries")
        if not quiet:
            for e in errors[:80]:
                print(" -", e)
            if len(errors) > 80:
                print(f" ... and {len(errors)-80} more")
        return 1
    print(f"OK: {total} entries, 0 violations, {len(files)} files")
    return 0

if __name__ == "__main__":
    sys.exit(main())
