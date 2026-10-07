"""vendor 规则数据脱敏（M3-1）：删掉 Nosey Parker 内建规则 YAML 里的
`examples` / `negative_examples` 段。

**为什么**：那 249 行里全是"长得像密钥"的示例（AWS / SendGrid / Mailgun / Twilio …）。
GitHub 的 push protection 不认"这是上游文档示例"，直接判 `Push cannot contain secrets`
拦下整条分支（2026-10-07 实测）。而本仓自己的纪律也是"夹具假值不留字面量"
（`.gitleaks.toml` 的白名单是给它没法避免的场景用的，不是给能删的场景用的）。

**行为影响：无**。匹配只用 `pattern`；`examples`/`negative_examples` 是规则 AST 里的文档字段
（`noseyparker-rules/src/rule.rs` 的 `RuleSyntax`，构造示例里就写着 `examples: vec![]`）。

**复核**：① 本脚本 `--check` 保证段已删净；② 真正的行为复核是 adv-secrets 的金丝雀——
它要加载并解析整包内建规则（解析不过就会失败），等于每天在 CI 上替这份脱敏背书；
③ VENDOR.md 记着上游提交号：重新同步上游后**必须重跑本脚本**（这是与上游唯一的差异）。
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RULES = ROOT / "third_party" / "noseyparker" / "crates" / "noseyparker" / "data" / "default" / "builtin" / "rules"

KEY_RE = re.compile(r"^ {2}(examples|negative_examples):\s*$")
ITEM_RE = re.compile(r"^ {2,}-\s")


def redact_text(text: str) -> tuple[str, int]:
    out: list[str] = []
    dropped = 0
    skipping = False
    for line in text.splitlines(keepends=True):
        if KEY_RE.match(line.rstrip("\r\n")):
            skipping = True
            dropped += 1
            continue
        if skipping:
            body = line.rstrip("\r\n")
            indent = len(body) - len(body.lstrip()) if body.strip() else 0
            # 列表项、块标量续行（缩进 > 2）、段内空行**与注释**都归这一段。
            # 实测教训两连：① examples 有用 `- |` 带多行 JSON 的，只删键与首行会留下悬空缩进；
            # ② auth0.yml 的 `examples:` 与它的 `- |` 之间夹着注释行，把注释当"段结束"
            # 会把整段块标量留在原地（auth0.yml:27 did not find expected key，金丝雀两连判红）。
            if ITEM_RE.match(line) or indent > 2 or not body.strip() or body.lstrip().startswith("#"):
                dropped += 1
                continue
            skipping = False
        out.append(line)
    return "".join(out), dropped


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    check = "--check" in argv
    if not RULES.is_dir():
        print(f"VENDOR-REDACT FAIL: 找不到规则目录 {RULES}")
        return 2
    files = sorted(RULES.glob("*.yml"))
    if not files:
        print(f"VENDOR-REDACT FAIL: {RULES} 下没有 .yml（上游结构变了？）")
        return 2
    changed = 0
    dropped = 0
    leftover: list[str] = []
    for f in files:
        text = f.read_text(encoding="utf-8")
        if KEY_RE.search(text) or any(KEY_RE.match(l.rstrip("\r\n")) for l in text.splitlines()):
            leftover.append(f.name)
        new, n = redact_text(text)
        if new != text:
            changed += 1
            dropped += n
            if not check:
                f.write_text(new, encoding="utf-8", newline="\n")
    if check:
        if leftover:
            print(f"VENDOR-REDACT FAIL: {len(leftover)} 个文件仍有 examples 段（{'、'.join(leftover[:5])}…）")
            return 1
        print(f"VENDOR-REDACT OK（{len(files)} 个规则文件，无 examples/negative_examples 段）")
        return 0
    print(f"VENDOR-REDACT 已脱敏：{changed} 个文件、删 {dropped} 行（重跑 --check 复核）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
