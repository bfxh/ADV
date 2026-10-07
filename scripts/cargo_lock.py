"""cargo-lock：根工作区 `Cargo.lock` ↔ `Cargo.toml` 声明的**漂移**判红。

为什么单列这一步（成因是门归属空洞，不是"再加一道保险"）：仓里已经有两道名字像它的门，
都不管这件事——① `deps-lock`（scripts/deps_lock.py）管 `rust/Cargo.toml` 那个**零依赖**旧目录
加 python CI 依赖的登记/钉版；② `xtask` 的「版本锁步」（lockstep.rs）管 workspace 版本 ↔
`adv-v*` tag。根 `Cargo.lock` 的新鲜度**没有主人**，而 CI 的 cargo 步骤又不传 `--locked`
（实测 `.github/workflows/adv.yml` 里 grep 不到 locked）。后果实测兑现过一次：M3-1 把
`adv-secrets` 接进 `adv-cli` 后，提交里的 Cargo.lock 少了这条依赖，本地门与 CI 全绿带过。

判据口径锚真实测量（2026-10-07，同一棵树两态各跑一遍）：
    漂移态  cargo metadata --locked               rc=101  310ms
    漂移态  cargo metadata --locked --no-deps     rc=0    252ms   ← 假绿陷阱：--no-deps 不校验锁
    漂移态  cargo metadata --locked --offline     rc=101  329ms
    修复态  cargo metadata --locked               rc=0    425ms
所以钉死为**不带 --no-deps 的 `--locked`**。刻意不用 `--offline`：CI 冷缓存时它会以
"failed to download"报红，那是环境红不是漂移红——常年环境红的门等于没有门。

退出码三态（与 mutants 的 exit 3、hook-status 的三态同一条纪律）：
    0 绿 / 1 漂移红 / 2 判不了（cargo 不可用、根不对、其他 rc）
2 也算「不绿」，但日志单独点名，别和真漂移混成一类账。

用法：python -X utf8 scripts/cargo_lock.py [--root <仓库根>]
"""
from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# cargo 对锁不新鲜的抱怨都带这句（"cannot update the lock file … because --locked was
# passed" / "the lock file … needs to be updated but --locked was passed"），按它分类。
DRIFT_MARK = "--locked was passed"


def verdict(root: pathlib.Path) -> tuple[str, str]:
    """跑一次 `cargo metadata --locked`，返回 (green|red|inconclusive, 说明)。"""
    if not (root / "Cargo.toml").is_file():
        return "inconclusive", f"{root} 下没有 Cargo.toml（--root 指错了吗）"
    cmd = ["cargo", "metadata", "--format-version", "1", "--locked"]
    try:
        cp = subprocess.run(cmd, cwd=str(root), capture_output=True, text=True,
                            encoding="utf-8", errors="replace")
    except (OSError, ValueError) as exc:
        return "inconclusive", f"cargo 跑不起来：{exc}"
    if cp.returncode == 0:
        return "green", "Cargo.lock 与声明同步"
    blob = (cp.stderr or "") + (cp.stdout or "")
    if DRIFT_MARK in blob:
        first = next((ln for ln in blob.splitlines() if DRIFT_MARK in ln), blob[:200])
        return "red", f"依赖锁漂移：{first.strip()}"
    return "inconclusive", f"cargo metadata rc={cp.returncode}：{blob.strip()[:200]}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT), help="仓库根（默认本仓；测试用临时合成仓）")
    a = ap.parse_args(argv)
    state, why = verdict(pathlib.Path(a.root).resolve())
    print(f"CARGO-LOCK {state.upper()}: {why}")
    return {"green": 0, "red": 1, "inconclusive": 2}[state]


if __name__ == "__main__":
    sys.exit(main())
