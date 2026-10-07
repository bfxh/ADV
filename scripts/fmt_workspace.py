"""fmt-workspace：只格式化**我们自己的 workspace 成员**（CI 与本地同尺）。

为什么单列这一步：`cargo fmt --all` 的官方语义是"所有包**以及它们的本地 path 依赖**"——
内化 Nosey Parker 之后（third_party/noseyparker，见其 VENDOR.md），实测它会把上游源码一起
格式化，diff 里全是 `third_party/**` 的文件。那等于逼我们改上游（vendor 口径当场破掉），
而且 CI 每轮必红。所以改成：从根 `Cargo.toml` 的 `[workspace].members` **现读**成员列表，
逐个 `cargo fmt --package <成员> --check`——成员增删自动跟上，不靠手工维护第二份名单。

退出码：0 全绿 / 1 有成员没格式化（逐成员点名）/ 2 用法或环境问题（读不到 members 等）。
"""
from __future__ import annotations

import pathlib
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent


def members(root: pathlib.Path = ROOT) -> list[str]:
    with (root / "Cargo.toml").open("rb") as f:
        data = tomllib.load(f)
    ms = (data.get("workspace") or {}).get("members") or []
    if not ms:
        raise SystemExit("fmt-workspace: 根 Cargo.toml 里读不到 [workspace].members")
    return list(ms)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    check = "--write" not in argv  # 默认只查；--write 才真格式化（本地修格式用）
    bad: list[str] = []
    for m in members():
        # 用 --manifest-path：members 里是**路径**（`crates/adv-core`），`-p` 要的是包名，
        # 拿路径当包名实测直接报 "not a member of the workspace"。
        cmd = ["cargo", "fmt", "--manifest-path", f"{m}/Cargo.toml"] + (
            ["--check"] if check else [])
        cp = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True,
                            encoding="utf-8", errors="replace")
        if cp.returncode != 0:
            bad.append(m)
            sys.stdout.write(cp.stdout)
            sys.stderr.write(cp.stderr)
    if bad:
        print(f"FMT-WORKSPACE FAIL: {len(bad)} 个成员未格式化（{'、'.join(bad)}）")
        return 1
    print(f"FMT-WORKSPACE OK（{len(members())} 个成员；上游 third_party/** 刻意不在面内）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
