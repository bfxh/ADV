"""S191 契约：存档区两条规则**走真路径**——MCP 工具面 → rx-scan.exe。

S187 的教训（判据必须走真路径）：内部函数自测只证明"匹配器会算"，证明不了
"工具面真的报得出来"。本文件只经 `registry.call("bug_scan")`：
- 违例夹具必须报出两条规则（含 severity/kind/行号）；
- 原子写 + 版本门夹具必须一声不响（金丝雀"静"的一头）；
- `by_rule` 聚合里必须出现新规则号（工具输出契约）。
Rust 侧语义细节（左词边界 / 掩码 / 测试区降级 / 确定性）由
`rust/tests/save_rules_test.rs` 承担；误报抽样账见 `bench/s191_save_rules_probe.py`。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import registry
import tools  # noqa: F401

SAVE_RULES = {"save_nonatomic_write", "save_load_no_version"}

# 直写存档 + 解析存档无版本门（两条各一处；行号 2 / 6 是判据的一部分）
VIOLATING = (
    "fn save_game(data: String) {\n"
    "    std::fs::write(\"save.ron\", data).unwrap();\n"
    "}\n"
    "\n"
    "fn load_game(text: &str) {\n"
    "    let data: SaveData = ron::from_str(text).unwrap();\n"
    "    apply(data);\n"
    "}\n"
)

# 中间路径 + rename（原子写）；format_version 分支（版本门）
CLEAN = (
    "fn save_game(data: String, save_path: std::path::PathBuf) -> std::io::Result<()> {\n"
    "    let save_tmp = save_path.with_extension(\"tmp\");\n"
    "    std::fs::write(&save_tmp, data.as_bytes())?;\n"
    "    std::fs::rename(&save_tmp, &save_path)\n"
    "}\n"
    "\n"
    "fn load_game(text: &str) -> Option<SaveData> {\n"
    "    let raw = ron::from_str::<RawSave>(text).ok()?;\n"
    "    if raw.format_version < CURRENT_SAVE_FORMAT {\n"
    "        return migrate(raw);\n"
    "    }\n"
    "    Some(upgrade(raw))\n"
    "}\n"
)


def _scan(tmp_path, name, body):
    (tmp_path / name).write_text(body, encoding="utf-8")
    r = registry.call("bug_scan", {"path": str(tmp_path)})
    assert r.get("ok"), r
    return r["result"]


def test_violating_fixture_is_reported_by_tool(tmp_path):
    """金丝雀"红"的一头：两条规则都必须经工具面报出，且行号对得上。"""
    res = _scan(tmp_path, "main.rs", VIOLATING)
    hits = {i["rule"]: i for i in res["issues"] if i["rule"] in SAVE_RULES}
    assert set(hits) == SAVE_RULES, res
    assert hits["save_nonatomic_write"]["line"] == 2, hits
    assert hits["save_nonatomic_write"]["severity"] == "med", hits
    assert hits["save_load_no_version"]["line"] == 6, hits
    assert hits["save_load_no_version"]["severity"] == "low", hits
    for i in hits.values():
        assert i["kind"] == "clue", i
    assert set(res["by_rule"]) >= SAVE_RULES, res["by_rule"]


def test_atomic_and_versioned_fixture_is_quiet(tmp_path):
    """金丝雀"静"的一头：原子写 + 版本门在场时一条都不许报。"""
    res = _scan(tmp_path, "main.rs", CLEAN)
    leftover = [i for i in res["issues"] if i["rule"] in SAVE_RULES]
    assert not leftover, f"原子写+版本门在场不得报：{leftover}"


def test_save_scan_is_repeatable(tmp_path):
    """同输入同输出（跨进程两跑逐条一致）——判据本身要能当判据。"""
    a = _scan(tmp_path, "main.rs", VIOLATING)
    b = _scan(tmp_path, "main.rs", VIOLATING)
    ka = [(i["rule"], i["line"]) for i in a["issues"]]
    kb = [(i["rule"], i["line"]) for i in b["issues"]]
    assert ka == kb, f"两跑必须一致：{ka} vs {kb}"
