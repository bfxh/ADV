"""S207 契约：LSP 探测表从 2 种扩到 23 种，以及「不卡」的会话上限。

由来（真实测量，不是凭印象）：本仓 `git ls-files` 的语言面是 json 462 / py 312 / rs 95 /
md 60，整个 D:\\KF 工作区 md 762 / json 136；而 `tools/lsp.py` 只接了 rust-analyzer +
pylsp，其余一律 not wired。扩容本身不难，难的是**别把宿主压死**——clangd / rust-analyzer /
jdtls 冷索引各吃数百 MB，23 种一起起就是"卡"的来源，所以本文件锁四件事：

1. **引用完整性**：扩展名表与文件名表指向的语言，必须真实存在于服务器表里（ghost 语言即红）；
2. **route() 三态如实分叉**：已接线 / 纯文本本就没有 LS 概念 / 探测表尚未收录——不假装支持，
   也不给一句"不支持"混过去；
3. **LRU 会话上限真的生效**：达到上限要停最久未用的那个，且停掉的进程必须真死；
4. **无扩展名文件**（Dockerfile 这类）命中——扩展名表天然做不到。

判据走真路径：起 `tests/fixtures/fake_lsp_server.py` 子进程驱动真客户端，不 mock 私有实现。
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import registry  # noqa: E402
from tools import lsp as lsp_mod  # noqa: E402

_STUB = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "fake_lsp_server.py")


@pytest.fixture()
def clean_sessions(monkeypatch, tmp_path):
    """每测隔离会话表 + 沙盒 + fake python 服务器；结束时把残留进程收干净。"""
    monkeypatch.setenv("UNIFIED_RX_LSP_CMD_PYTHON", f"{sys.executable} {_STUB}")
    monkeypatch.setenv("UNIFIED_RX_SANDBOX", str(tmp_path))
    monkeypatch.setattr(lsp_mod, "_SESSIONS", {})
    yield tmp_path
    for key in list(lsp_mod._SESSIONS):
        lsp_mod._SESSIONS[key][0].stop()
    lsp_mod._SESSIONS.clear()


def _mkproj(root, name):
    d = root / name
    d.mkdir(parents=True, exist_ok=True)
    f = d / "m.py"
    f.write_text("def target_fn():\n    return 42\n\n\ntarget_fn()\n", encoding="utf-8")
    return str(f), str(d)


def test_table_shape_and_reference_integrity():
    assert len(lsp_mod._LSP_SERVERS) >= 15, "探测表缩水——S207 扩容白做了"
    wired = set(lsp_mod._LSP_SERVERS)
    for lang, spec in lsp_mod._LSP_SERVERS.items():
        assert spec.get("label"), f"{lang} 没有 label"
        assert spec.get("hint"), f"{lang} 未装时没有安装提示（detected=false 得给下一步）"
        cmd = spec["cmd"]()
        assert isinstance(cmd, list) and cmd and all(isinstance(c, str) for c in cmd), \
            f"{lang} 的 cmd() 必须返回非空字符串列表"
    ghost = {v for v in lsp_mod._LANG_BY_EXT.values()} | set(lsp_mod._LANG_BY_NAME.values())
    assert ghost <= wired, f"扩展名表指向不存在的语言（ghost 锚）: {sorted(ghost - wired)}"
    exts = {e for e in lsp_mod._LANG_BY_EXT if e in (".txt", ".text", ".log")}
    assert exts == set(), f"纯文本不该被接进 LSP: {sorted(exts)}"


def test_route_three_states_are_distinguishable():
    assert lsp_mod.route("D:/x/Cargo.toml") == ("toml", "")
    assert lsp_mod.route("D:/x/main.rs")[0] == "rust"
    lang, why = lsp_mod.route("D:/notes/plan.txt")
    assert lang is None and "无语言服务器" in why, f"纯文本要如实说没有 LS 概念: {why!r}"
    lang, why = lsp_mod.route("D:/x/weird.qqq")
    assert lang is None and "未收录" in why and str(len(lsp_mod._LSP_SERVERS)) in why, \
        f"未收录要给出可行动信息（表规模+怎么查）: {why!r}"


def test_dockerfile_without_extension_resolves():
    assert lsp_mod.route("infra/Dockerfile")[0] == "dockerfile"
    assert lsp_mod.route("deploy/containerfile")[0] == "dockerfile"


def test_session_cap_evicts_least_recently_used(clean_sessions, monkeypatch):
    monkeypatch.setenv("UNIFIED_RX_LSP_MAX_SESSIONS", "2")
    assert lsp_mod._session_cap() == 2
    roots = [_mkproj(clean_sessions, f"p{i}")[1] for i in (0, 1, 2)]
    first = lsp_mod._get_session("python", roots[0])
    first.last_used = 0.0                      # 固定 LRU 顺序，不靠墙钟时序赌运气
    second = lsp_mod._get_session("python", roots[1])
    assert len(lsp_mod._SESSIONS) == 2
    third = lsp_mod._get_session("python", roots[2])
    assert len(lsp_mod._SESSIONS) <= lsp_mod._session_cap(), "超上限还继续起——上限是装饰"
    assert not first.alive(), "最久未用的会话没被停：多后端并发会把宿主压穿"
    assert second.alive() and third.alive(), "LRU 停错了对象（该留最近在用的）"
    assert (third.lang, third.root) in lsp_mod._SESSIONS


def test_status_reports_surface_and_undetected_hint(clean_sessions):
    r = registry.call_with_context("ide_lsp", {"action": "status", "file": None}, request_id="t-s207")
    assert r["ok"], r
    surface = r["result"]["servers"]["_surface"]
    assert surface["servers"] == len(lsp_mod._LSP_SERVERS)
    assert surface["sessions_cap"] == lsp_mod._session_cap()
    for lang, entry in r["result"]["servers"].items():
        if lang == "_surface":
            continue
        assert "detected" in entry, f"{lang} 状态缺字段"
        if not entry["detected"]:
            assert entry.get("reason") and entry.get("hint"), f"{lang} 未命中却没给原因/安装提示"
