> 2026-10-16 并入 ADV 工作区（原独立仓 bfxh/uiscan 已归档）。

# uiscan

把"**屏幕上真的出现了哪些文案**"抓出来，用来查静态扫描查不到的那类缺陷：硬编码/漏翻、
截断、重叠、以及**缺字画成方框（豆腐块）**。Rust 写的，判据全是纯函数 + 真机语料。

## 为什么需要它（一句话）
静态 i18n 扫描器看的是源码，可及性树看的是"作者声明的逻辑字符串"，两边都不能告诉你
**用户实际看见了什么**。本项目的主判据就是这两者的差集。

## 三条已实测的表面事实（本仓判据的地基，`tests/` 里逐条钉住）
| 事实 | 实测口径 | 后果 |
|---|---|---|
| `::before`/`::after` 的 computed content **不带引号**，`::marker` 的**带引号** | Chromium 150（Qoder 内置 webview）真跑 `getComputedStyle` | 剥引号逻辑不能只按规范写，两边不对称 |
| `counter(i) ". "` 回来是**未解析令牌**；`attr()`/`var()` 回来是**已解析值** | 同上 | 只有还带函数括号的才算噪音，`counter` 必须丢 |
| CSS `::placeholder{content:…}` 实测被忽略（回来 `normal`） | 同上 | 占位文案只走 `placeholder` **属性**通道 |
| `display:none` 宿主的伪元素 content 照样给值 | 同上 | 不过滤几何就会报"用户看不见的文案" |
| `→ ← ✓ ✗` 在终端里判**单宽**，中文/全角判双宽 | vt100 0.16.2 真跑 | 宽字符续格必须跳过；符号也要进覆盖检查 |
| UIA 顶层窗口里有 `(-9999,-9999)` 屏外哨兵与 `(0,0)` 零尺寸 | `windows 0.62.2` on **x86_64-pc-windows-gnu** 真跑 | 矩形不可信的一律不进结果 |

## 两条口径分开的门（不是冗余）
- `Harvest::candidates()` —— 漏翻比对用：可见 **且** 像文案（有字母、非生成器令牌）。
- `Harvest::rendered_codepoints()` —— 豆腐块/字体覆盖用：**所有可见**条目，含纯符号。
  这条分开是测试逼出来的：拿候选集查覆盖会漏掉 `✓ ✗ →` 整类缺字。

## 现状（不含糊）
- ✅ `uiscan-core`：模型 + 判据（9 条测试）
- ✅ `uiscan-tui`：vt100 采集（5 条测试 + CLI 端到端跑通，21 个非 ASCII 码点与探针对得上）
- ✅ `uiscan-web`：**已接 CDP 并真跑过**。本机没有 `msedgedriver`，所以不走 WebDriver：
  由工具自己起 `msedge --headless=new --remote-debugging-port`，读 `/json/list` 挑页、连 ws 跑
  `HARVEST_JS`。真跑读数：Edge `155.0.4283.45`、viewport `[1256,808,1256]`、12 项植入期望全中；
  语料 `tests/fixtures/payload-edge155.json` 上钉了 6 条回归（含"隐藏元素进采集但不进候选"）。
  接通过程中踩到并修掉的三个真 bug（都写进代码注释了，别重犯）：
  1. `Runtime.evaluate` 直接喂箭头函数源码只会得到**函数对象**（`returnByValue` 序列化成 `{}`）⇒
     必须包成 `(fn)()`。症状很阴：`rc=0`、viewport 为 Null、0 条。
  2. CDP 的 page 目标**早于 DOM 就绪**就出现 ⇒ 立刻采会得到"viewport 正常、items 为空"的**假成功**；
     必须先等 `document.readyState === "complete"`。
  3. DevTools 的 HTTP 端点**不理会 `Connection: close`**，`read_to_end` 会一路挂到读超时（10060）；
     必须按 `Content-Length`/chunk 边界读够就收。（我一度以为是它校验 `Host` 头——那只是假设，没验过，作废。）
- ✅ `uiscan-gui`：UIA 适配器已写并在真桌面跑过（`uiscan gui --list | --title <子串> | --pid N | --focused`）。
  `windows 0.62.2` 在 **x86_64-pc-windows-gnu（无 MSVC）** 编链并跑通；深度/节点数有硬闸；
  纯映射层（ControlType 命名、路径段、RECT 换算、可见性）单独可测，非 Windows 是**失败桩**（不是空结果）。
  - 🔴 **实测边界（2026-10-16，fmnr = egui 应用）**：UIA 树里只有窗口名 + 标题栏四个按钮
    （`系统`/`最小化`/`最大化`/`关闭`），**egui 自绘的界面内容一个节点都没有**。
    所以即时模式 GUI 的正文在这条通道上不可见——它的豆腐块/截断只能靠字体覆盖与像素那两层。
    任何"AccessKit 应该能看到"的说法都要先在这台机器上用真窗口验过再说。
  - UIA 的 `Name` 是**作者声明的逻辑串**：缺字体画成方框、被容器裁掉，树里都看不出来。
    这一面能判的是：空名、屏外/零尺寸、以及把 Name 拿去和翻译记忆对账（漏翻/硬编码）。
  - ControlType 编码分两档记录：实测确认的（50030/50032/50033/50037/50010）与按文档的其余值；
    **认不得的一律 `Unknown(数值)`，不猜**（第一版把 50037 猜成 "List?" 是错的）。
- ⬜ 截断/重叠判据：Web 面现在有真窗口尺寸了（`--window`），可以量；TUI 面用格宽；GUI 面等适配器。

## 跑
```
cargo run --bin uiscan -- tui-feed --file <终端字节捕获> --rows 24 --cols 80
cargo test --workspace
```
未知子命令、未接的面、缺参数一律 **退 2 并点名**，不返回空结果假装成功。
