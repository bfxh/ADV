# FP 会计账 — M1 片1（2026-10-04）

> 验收口径（蓝图 §10 M1）：对真实仓跑通首批规则；FP 会计逐条对上。
> 规则引擎 = adv-parse（tree-sitter 0.25，Python+Rust 双语归一 AST）+ adv-rules（5 条 YAML 规则）+ adv-cli `scan`。
> 复现：`cargo run -p adv-cli -- scan <目录> [--exclude vendor]`；规则测试门禁 = `cargo test -p adv-rules`（ruleid 注解逐行一致）。

## 靶 1：ADV 自身（crates/ + xtask，25 文件）——**11 条全量逐条**

| # | 位置 | 规则 | 判定 | 理由 |
|---|------|------|------|------|
| 1 | crates/adv-parse/src/lib.rs:32 | RS-UNWRAP-USE | TP（设计内） | 测试代码 unwrap（断言入口），测试域惯用 |
| 2 | crates/adv-parse/src/lib.rs:45 | RS-UNWRAP-USE | TP（设计内） | 同上 |
| 3 | crates/adv-parse/src/lib.rs:59 | RS-UNWRAP-USE | TP（设计内） | 同上 |
| 4–11 | crates/adv-rules/tests/ruleid.rs:11,53,54,55,56,64,77,85 | RS-UNWRAP-USE | TP（设计内） | 测试代码（8 处 unwrap 全在 #[test]） |

**结论**：产品代码 0 条发现；11 条全部落在测试代码 = TP-by-design。豁免机制（行内抑制四要素：粒度/规则码/原因/到期）是片2 交付——在那之前这些就是规则引擎自身的"已知真阳性账"。

## 靶 2：VoxelForge-V3（真实仓规模靶，392 文件 → `--exclude vendor` 后 239 文件）

**vendor/ 教训（本片最大收获）**：不排除 vendor 时 1008 条中 230 条来自 `vendor/avian3d`（第三方引擎，panic/unwrap 是上游设计）——真实仓首跑直接教出 `--exclude` 作用域需求，已实现。

全量分布（排除 vendor 后 778 条）：

| 规则 | TEST | PROD |
|------|------|------|
| RS-UNWRAP-USE | 502 | 199 |
| RS-PANIC-USE | 71 | **5（全量逐条如下）** |
| PY-EXEC-USE | 0 | **1（逐条如下）** |

### PROD panic 5 条全量逐条

| 位置 | 上下文 | 判定 |
|------|--------|------|
| crates/core/src/assembly/layout.rs:238 | `.unwrap_or_else(\|_\| panic!("放置 {i} 失败"))` | TP：带消息的不变量断言，故意为之 |
| crates/core/src/assembly/layout.rs:324 | `PartShape::Wheel => panic!("结构格不应是轮")` | TP：穷举匹配不可能臂断言 |
| crates/server/src/reload.rs:149 | let-else `panic!("非拒绝回执必有 ModulePlaced 广播")` | TP：协议不变量断言 |
| crates/server/src/solver.rs:321 | let-else `panic!("({x},{z}) 未命中地形")` | TP：不变量断言（可改 Result，属 VF3 工程债非规则误报） |
| crates/server/src/solver.rs:387 | `panic!("结构格不应是轮圆柱")` | TP：穷举匹配不可能臂断言 |

**PROD panic 误报率 0/5**；规则忠实命中"产品代码 panic!"，去留是 VF3 的工程决策。

### PY-EXEC-USE 1 条

`tools/test_plugin_headless.py:28` — TP：VF3 的插件测试工具脚本用 exec 加载插件；规则判定正确。

### PROD unwrap 199 条——**明示不逐条**（判定工程量出界，诚实记档而非抽样升总体）

分布（Top）：assembly/mod.rs 49、assembly/patch.rs 39、dig.rs 18、content/registry.rs 10、infra/csvdata.rs 10……共 24 文件。
后续片次可按 crate 分批全量判定（每批出独立小账），本片不做。

## 规则语义裁定（开发中被门禁抓出来的）

1. **裸名 ≠ 方法调用**：ruleid 门禁首跑抓到 `obj.eval(value)` 被 `eval` 规则误报 ⇒ 语义定为三段：裸名=只命中裸名；前导点 `.unwrap`=方法调用末段；点分路径=整段精确。
2. **vendor/ 作用域排除**：`--exclude` 按路径组件匹配（上）。
3. **残留调试文件被抓**：自查 12 条里 1 条是忘删的 examples/dbg.rs——扫描器抓到了自己人生成的垃圾（已删）。

## 已知限制（片2 清单）

- 导入别名解析缺失（`from subprocess import run` 后裸调 `run(shell=True)` 漏报——FN 面，片2 导入感知）
- 多导入语句只记首个模块名；宏覆盖只到 `name!` 形态；非 UTF-8 文件跳过（计数在 stderr）
- 匹配器朴素全遍历（aho-corasick 预过滤等 RESEARCH X4 手法，待基线后按需加——先量后改）

## 下一步（片2）

YAML taint spec（sources/sanitizers/propagators）+ 进程内污点追踪 + 抑制四要素 + 导入感知解析。
