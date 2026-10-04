# FP 会计账 — M1 片2（2026-10-04）

> 片2 交付：YAML taint spec（sources/sinks/propagators/sanitizers）+ 进程内污点跟踪 +
> 行内抑制四要素（粒度=行/规则码/reason/until）+ 导入感知解析。
> 复现：`cargo test -p adv-rules`（夹具 = tests/m1s2.rs，ruleid 注解逐行一致）。

## 判据（全部 ruleid 门禁逐行对上，tests/m1s2.rs）

| 场景 | 结果 |
|------|------|
| `s = input(); eval(s)`（源→变量→汇点） | 命中 ✓ |
| `t = s.strip(); system(t)`（传播子 + from-import 裸调汇点） | 命中 ✓ |
| `os.system("cmd " + s)`（表达式内污点扩散） | 命中 ✓ |
| `n = int(input()); eval(n)`（净化子） | 不命中 ✓ |
| `eval("1+1")`（干净字面量） | 不命中 ✓ |
| `run("ls", shell=True)` after `from subprocess import run` | 命中 ✓（片1 的 FN 面闭） |
| 有效抑制（reason+until 齐全） | 发现被吃掉 ✓ |
| 过期抑制（until < 今日） | 发现照报 ✓ + 到期账单列 ✓ |
| 畸形抑制（缺 until） | 进畸形账，发现照报，不静默 ✓ |

## 开发中被判据抓出来的三个真 bug（本轮最值钱的部分）

1. **方法调用漏 receiver**：`t = s.strip()` 的污点传播失败——方法的**接收者**不在 args 里，
   在函数路径的基座（children[0]）。污点跟踪补 receiver 入口。
2. **taint 集合判定没走三段语义**：`strip` 作为方法传播子需要 `.strip` 形态（前导点），
   裸名精确匹配命中不了 `s.strip`；且汇点/源/传播/净化全部要吃导入解析名
   （`from os import system` 后裸调 `system`）。第一版只把导入表接给了 call 规则。
3. **导入建账把 module 本身混进 names**：`from os import system` 建出 `os→os.os` 假账
   （module_name 字段的孩子被当成导入项）；且该语法的导入项是 `dotted_name` 不是 `name`。

## 语义裁定（与片1 同一血统）

- taint 集合判定与 call 规则共用 callee_matches 三段语义（裸名/前导点/点分精确）。
- 抑制 = 行级四要素：reason 与 until 必填，缺失/畸形**不静默**（发现照报 + 畸形账）；
  到期（until < 今日）发现照报 + 到期账（防永久豁免——旧仓 noqa 永久豁免教训）。
- Match 形态 `call`/`taint` 二选一：都缺或都在 = 加载即红（serde_yaml_ng 对外部 tagged
  枚举要求 YAML tag 而非 map 键 ⇒ 显式 Option + 加载期校验，fail-closed 更直白）。

## 已知限制（片3 清单）

- 污点为进程内单文件、语句序流敏感；跨函数/跨文件传播不做。
- 未声明 propagators 的调用不吃污点（保守，FN 面存在：如 `s.replace(...)` 未声明则丢污点）。
- 抑制粒度只有行级；文件级/范围级与 `#[expect]` 式到期门禁（xtask 到期账）留片3。
- rust 侧 use 树解析不展开别名（`as`）；rust 污点规则不支持。
- 匹配仍朴素全遍历（预过滤待基线）。

## 下一步（片3）

抑制到期账进 xtask 门（过期即红，带时间戳豁免）+ rust use 别名 + 污点跨语句增强（关键字参数传播）。
