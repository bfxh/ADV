"""hub_manifest.py —— manifest 的声明字段（S187，spec/FRONTIER-CI.md 方向③）。

**判定：有界。** 用户方向里的"带版本/依赖/测试的跨平台 CI 模块生态"，本仓只需吸收
"声明"这一半：`version`（本管线自己的版本标签）与 `requires`（依赖声明）。另一半——
依赖解析、注册中心、跨平台编译——**不做**：跨平台腿已由 CI 矩阵承载，而"模块带版本"
git tag/commit 就够，不必造包管理器。

**没有解析器，所以不假装有**：这里只验**格式**（`_split_req`），可满足性只判**存在性**
（`missing_requires`）。带 `@版本` 的条目一律如实标「版本未解析」——假装的版本校验比不写
更糟（验证者会以为约束被满足过）。

本模块自 `hub_core.py` 拆出：manifest 校验与运行账本/护栏是两件事，而 god 门棘轮对
`hub_core.py` 只准减不增（S187 拆分即为"合法交换"的同一精神）。
"""
from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import tempfile

_ID_RE = re.compile(r"[a-z][a-z0-9]*(\.[a-z0-9-]+)+")
_VER_RE = re.compile(r"\d+(\.\d+){0,3}")


def _fixture(pid: str, **extra) -> dict:
    """合法 manifest 夹具（本模块最清楚自己的字段集，故自带；命令用白名单内字面量）。"""
    return {"id": pid, "title": "t", "when": "t", "resource_class": "shared",
            "on": ["manual"], "steps": [{"step": "s", "cmd": ["python", "-c", "print(1)"],
                                        "timeout_s": 60}], **extra}


def _split_req(pid: str, entry: str) -> tuple[str, str]:
    """拆 `id` / `id@版本`；**只验格式，不做约束求解**（本仓无包管理器，不假装能解）。"""
    rid, _, rver = entry.partition("@")
    if not _ID_RE.fullmatch(rid):
        raise ValueError(f"{pid}: requires 条目 {entry!r} 的 id 形如 域.场景（小写点分）")
    if rver and not _VER_RE.fullmatch(rver):
        raise ValueError(f"{pid}: requires 条目 {entry!r} 的版本形如 1.2（可省）")
    return rid, rver


def check_labels(pid: str, data: dict) -> None:
    """可选声明字段 `version`/`requires`：**校验器认格式，不引解析器**。

    - `version`：形如 1 / 1.2 / 1.2.3；
    - `requires`：条目为 `id` 或 `id@版本`；不得自引用、不得重复。
    """
    ver = data.get("version")
    if ver is not None and (not isinstance(ver, str) or not _VER_RE.fullmatch(ver)):
        raise ValueError(f"{pid}: version 形如 1 / 1.2 / 1.2.3（可省）")
    req = data.get("requires")
    if req is None:
        return
    if not isinstance(req, list) or not all(isinstance(x, str) and x for x in req):
        raise ValueError(f"{pid}: requires 必须是字符串列表")
    if len(set(req)) != len(req):
        raise ValueError(f"{pid}: requires 有重复条目")
    for entry in req:
        rid, _rver = _split_req(pid, entry)
        if rid == pid:
            raise ValueError(f"{pid}: requires 不得自引用")


def missing_requires(pipes: dict[str, dict]) -> dict[str, list[str]]:
    """依赖声明的**可满足性**：只做存在性——id 不在本目录 ⇒ 如实列出；带 `@版本` 的条目
    不做满足判定 ⇒ 如实标「版本未解析」。宁少报语义，不假装校验过。"""
    out: dict[str, list[str]] = {}
    for pid, m in sorted(pipes.items()):
        gaps: list[str] = []
        for entry in m.get("requires") or []:
            rid, rver = _split_req(pid, entry)
            if rid not in pipes:
                gaps.append(f"{entry}（缺 {rid}）")
            elif rver:
                gaps.append(f"{entry}（版本未解析）")
        if gaps:
            out[pid] = gaps
    return out


def _rejected(patch: dict) -> bool:
    """非法声明必须被拒——否则"字段被认"可以只是收下就完事（假实现）。"""
    import hub_core  # 延迟导入：hub_core 在模块级 import 本模块
    try:
        hub_core.validate_manifest("adv.meta.json", _fixture("adv.meta", **patch))
    except ValueError:
        return True
    return False


def _disk_facts() -> dict[str, bool]:
    """磁盘 round-trip：`load_pipelines` 是否**真**把 version/requires 带出来。

    只验"校验函数存在"是不够的——字段没接到加载路径上，功能就是不生效。
    """
    import hub_core
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-meta-"))
    pipes = tmp / "pipelines"
    pipes.mkdir(parents=True)
    (pipes / "adv.meta.json").write_text(
        json.dumps(_fixture("adv.meta", version="1.0", requires=["adv.nobody@1.0"])),
        encoding="utf-8")
    saved = {k: os.environ.get(k) for k in ("UNIFIED_RX_HUB_ROOT", "UNIFIED_RX_HUB_PIPELINES")}
    try:
        os.environ["UNIFIED_RX_HUB_ROOT"] = str(tmp / "hub")
        os.environ["UNIFIED_RX_HUB_PIPELINES"] = str(pipes)
        pm, inv = hub_core.load_pipelines()
        return {"loaded_with_fields": (pm.get("adv.meta") or {}).get("version") == "1.0"
                and not inv,
                "gaps_on_disk": bool(missing_requires(pm).get("adv.meta"))}
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        shutil.rmtree(tmp, ignore_errors=True)


def meta_facts() -> dict[str, bool]:
    """方向③ 的判据**事实**（布尔表；判定口径——全真才绿 + 逐项可读——由门统一负责）。"""
    import hub_core
    out: dict[str, bool] = {}
    hub_core.validate_manifest("adv.meta.json",
                               _fixture("adv.meta", version="1.2.3",
                                        requires=["adv.other", "adv.two@1.0"]))
    out["accepts_declared"] = True
    bad = ({"version": "v1.2"}, {"version": "1.2.3.4.5"}, {"version": 12},
           {"requires": "adv.x"}, {"requires": ["Adv.X"]}, {"requires": ["adv.meta"]},
           {"requires": ["adv.x", "adv.x"]}, {"requires": ["adv.x@v1"]})
    out["rejects_all_invalid"] = sum(1 for p in bad if _rejected(p)) == len(bad)
    gaps = missing_requires({"adv.meta": {"requires": ["adv.other", "adv.two@1.0"]},
                             "adv.two": {"requires": []}})
    joined = " ".join(gaps.get("adv.meta") or [])
    out["missing_reported"] = "adv.other" in joined
    out["version_not_faked"] = "未解析" in joined     # 有解析器就会"解出来"，我们没有
    out["clean_decl_no_gap"] = gaps.get("adv.two") is None
    out.update(_disk_facts())
    return out
