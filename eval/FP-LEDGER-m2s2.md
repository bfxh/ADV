# FP 会计账 — M2 片2（2026-10-04）

> 片2 交付：**同文件跨函数污点传播**（函数摘要三旗标：returns_source / returns_param_taint /
> sink_with_param）+ **作用域隔离**（每函数独立变量状态，修掉片1 平铺 state 跨函数泄漏）。
> 污点逻辑从 matcher.rs 抽出为 taint.rs（模块化；matcher 只留 call 匹配与分发）。
> 复现：`cargo test -p adv-rules --test m2s2`（ruleid 注解逐行一致）。

## 判据（全量逐行）

| 场景 | 结果 |
|------|------|
| `get_input()` 体内引用源、`system(s)` 跨函数吃返回值 | 命中 ✓（returns_source 摘要） |
| `wrap(x){ return system(x) }`，调用点 `wrap(s)` 且 s 污染 | 命中 ✓（param→sink 摘要，**报在调用点行**） |
| `passthrough(x){ return x }` 接源后入汇点 | 命中 ✓（returns_param_taint 摘要） |
| `clean(x){ return int(x) }` 净化函数 | 不命中 ✓ |
| 同名变量跨函数（handler 污染 / unrelated 字面量） | 不命中 ✓（作用域隔离生效） |

## 开发中被判据抓出来的真 bug

**参数提取器对 FunctionDef 节点不递归**——`walk_params` 只匹配 parameters/Identifier
两种节点，FunctionDef 本身直接落空 ⇒ 参数永远收不到 ⇒ 三个摘要旗标全部失灵，
三个测试全红。修法：只在 FunctionDef 孩子里定位 parameters 节点、在该子树内收
Identifier（默认值表达式标识符会被多收——保守方向，记档）。

## 语义裁定

- 摘要按文件序单遍：**嵌套本地函数调用不参与摘要解析**（后定义函数对先定义者不可见）；
  递归不做不动点。同函数名跨类合并（保守）。
- 跨函数流报在**调用点行**（不是函数定义行）——报告位置=行动位置。
- 摘要内部不做 sink 判定的本地函数调用解析（round-1 无摘要可用）——FN 面记档。

## 已知限制（后续片）

- 跨文件传播不做（文件级摘要导出/导入是 M3+ 形态）。
- 类方法（python class 内 FunctionDef）按函数名合并摘要，可能过报（保守）。
- 参数默认值表达式标识符被当参数种子（过拟，保守）。

## 下一步

按蓝图：M2 深轨边车（nightly + rustc_private + MIR IFDS，验收=与旧 Rust 引擎对拍）。
