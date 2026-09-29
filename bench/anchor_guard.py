"""anchor_guard.py —— bench 测量锚的可用性声明与强制（S198）。

**为什么**：`D:\\开发\\VoxelForge-V3` 等真值树已随盘迁移消失，但 bench 脚本仍会
"跑成功"——`truth_exists` 全 False 产出一版**无意义但格式正常**的数字（量空气）。
本守卫把"锚必须在场"变成硬前置：缺锚 ⇒ 退出码 2 + `ANCHOR-MISSING`，拒绝产出。
登记表在 `spec/bench-anchors.json`，由门链 `bench-anchor` 步复算（detached 锚的
每个 consumer 必须调用 `require_anchor`，未登记的新外部路径 ⇒ 门红）。

**纪律（S198 实锤第二课）**：`require_anchor` 只许在**脚本入口**（main/__main__）
调用，不许放模块级——`tests/test_ab_runner.py` 与 `swe_p3/swe_repair` 会 import
bench 模块，模块级校验会拿 SystemExit 打死收集期（一次全量 pytest INTERNALERROR
的成因）。模块级常量用 `env_anchor`（纯函数，无副作用）。
"""
from __future__ import annotations

import os
import sys


def env_anchor(env: str | None, default_path: str) -> str:
    """纯解析：env 覆盖优先，否则默认路径。**不校验存在性**——供模块级常量用。"""
    v = (os.environ.get(env) or "").strip() if env else ""
    return v or default_path


def require_anchor(anchor_id: str, path: str) -> str:
    """入口校验：目录不在场即 ANCHOR-MISSING 退出（不静默、不产数字）。"""
    if not os.path.isdir(path):
        sys.stderr.write(
            f"ANCHOR-MISSING anchor={anchor_id} path={path}\n"
            "  拒绝量空气：重建锚目录或设 env 覆盖后复跑；既有账面数字=当时时点值，非当前可复算。\n")
        sys.exit(2)
    return path
