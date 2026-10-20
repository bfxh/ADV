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

## 豆腐块判据（`uiscan tofu`，2026-10-16）

判据 = **码点在字体栈里有没有字形**（`ttf-parser` 读 `cmap` 的 `glyph_index`），不是 OCR、不是截图。

```
uiscan tui-feed --file cap.bin --rows 6 --cols 40 --json tui.json
uiscan tofu --json tui.json --font C:\Windows\Fontsrial.ttf     # 只给拉丁字体
uiscan tofu --json tui.json --font C:\Windows\Fonts\msyh.ttc      # 只给中文字体
uiscan tofu --json tui.json --system                                # 全系统字体并集
```

退出码：**0 = 全有覆盖；1 = 发现缺字**（判据成立的样子，不是工具坏了）；2 = 用法/环境错。

真跑读数（同一份 TUI 采集，21 个非 ASCII 码点）：

| 字体栈 | 面数 | 缺字 | 说明 |
|---|---|---|---|
| `arial.ttf` | 1 | **19** | 汉字、`✓`/`✗` 全缺——这就是 fmnr 那类"拉丁字体栈下中文全豆腐块"的复现 |
| `msyh.ttc` | 2 | **2** | 只剩 `✓`(U+2713) / `✗`(U+2717) |
| `--system` | 178 | **0** | 系统字体并集能覆盖全部；155/156 个文件可解析 |

**边界（必须传对字体栈）**：这是"字体能力"检查，不是像素验证。应用若只内嵌了某套字体、根本不做系统回退
（egui 默认字体即如此），就必须把**它实际用的字体栈**传进来；传系统并集会得到"没问题"而屏幕上仍是方框。

## 一条命令：`uiscan scan`（2026-10-16）

```
uiscan scan --surface tui --file cap.bin --rows 6 --cols 40 --system
uiscan scan --surface web <url> --window 1280x900 --font <字体>
uiscan scan --surface gui --title <子串> --depth 10 --system
```
`--surface` 之后的参数**原样交给对应面**（与 `tui-feed`/`web`/`gui` 走同一份采集代码），
另加三个共享开关：`--font`（可重复）、`--system`、`--json`（归一化 dump）。
退出码与 `tofu` 一致：0=全有覆盖、1=发现缺字、2=用法/环境错。

真跑验证（同一台机）：
- `scan --surface web …i18n-pseudo.html --system` ⇒ 22 个码点 / 178 面 / **缺字=0**；
- `scan --surface gui --title 任务栏 --system` ⇒ 80 个码点（含 U+200E）/ 178 面 / **缺字=0**；
- `scan --surface tui --file …` 三条端到端契约：无字体退 2、只给 arial 判出 U+4FDD 退 1、`--system` 退 0。

## 真实项目跑了一次（青简官网 + fmnr，2026-10-20 本机时钟）

| 目标 | 面 | 结果 | 结论 |
|---|---|---|---|
| `https://qingjian.app`（真站点） | web(CDP) | 328 元素 / 93 DomText + 5 TitleAttr / **272 个非 ASCII 码点** / 178 字体面 / **缺字 0** | 站点中文全部有字形覆盖 |
| 同一站点，只给 `arial.ttf` | web(CDP) | **缺字 264**，rc=1 | 判据在真实项目上真的会红 |
| 本机 `fmnr`（egui 应用） | gui(UIA) | 只有窗口名 + 标题栏 4 个按钮（8 个码点），缺字 0 | **正文不在 UIA 树里**——这一面覆盖不到 egui 自绘内容，别把 0 当"没问题" |

### 真跑照出的三个缺陷（都已修，且补了测试）
1. **`about:blank` 抢先采**：Edge 起窗后先有一个 `about:blank` 的 page 目标，`readyState` 在它上面
   已经是 `complete` ⇒ 采到空页（`href=about:blank`、elements=3）。修法：等 `location.href` 真到目标。
2. **"必须 complete"在真实站点上站不住**：qingjian.app 卡在 `interactive` 30 秒（挂起的统计/长连接）。
   修法：就绪判据改为"href 到目标 + `readyState` 不是 loading"，再给 500ms 沉降。
3. **空采集被报成绿**：0 条时原来会打印"缺字=0、rc=0"——这正是假绿。修法：采集到 0 条一律 **退 2**
   并把 title/href/elements/body_len 打进消息里，让人一眼看出是"没渲染/被拦/选错面"。
4. 顺带补的：file:// 的 URL 比较要折叠 `/./`、`/../`（夹具路径带 `..` 时浏览器会归一化 href）。

## GUI 面为什么抓不到正文（2026-10-20 实测 A/B）

同一个最小 eframe 0.31 程序，只差 `eframe` 的 features：

| features | UIA 读数 |
|---|---|
| `["default_fonts","glow","accesskit"]` | **8 条**：窗口名 + 标题栏 4 项 + `Text[1]#测试文本 中文` + `Button[2]#按钮文字` |
| `["default_fonts","glow"]` | **6 条**：只有窗口名 + 标题栏（`akprobe` 实测） |

`fmnr`（eframe 0.31，`default-features=false`，features 里没有 `accesskit`）正好落在第二行——
所以它的正文"抓不到"不是遍历写错，而是**应用根本没把界面接到可及性树**。

**修法（应用侧，一行）**：在 `eframe` 的 features 里加 `"accesskit"` 后重建。Dear ImGui 没有可及性实现，
其它即时模式框架需要自接 AccessKit/UIA provider。本工具不替应用改依赖。

**工具侧已做的**：采到"只有系统装饰"（窗口自身 + TitleBar/MenuBar）时，`gui` 与 `scan` 都会往 stderr
打一条明确提示，说明"0 条正文 ≠ 界面没有文字"，并给出上面的处置方向；该判定有冻结语料测试
（`gui-fmnr.json` 判真、带正文的合成树判否、空采集不归类）。

## 截断判据（`uiscan clip`，2026-10-20）

判据 = 浏览器自己给的**布局几何**，不猜 CSS 语义。`HARVEST_JS` 现在给每条 `DomText` 附带
`sw/cw/sh/chh`（scroll/client 宽高）与 `ovx/ovy/ell`（overflow、text-overflow）。

实测（夹具 `truncation.html`，Edge 155 headless）：

| 形状 | sw/cw | sh/chh | overflow | text-overflow | 判 |
|---|---|---|---|---|---|
| 放得下 | 160/160 | 24/24 | visible | clip | — |
| 横向硬裁 | **336**/160 | 24/24 | hidden | clip | `Horizontal` |
| 横向省略号 | **336**/160 | 24/24 | hidden | **ellipsis** | `Ellipsis` |
| 纵向裁切 | 160/160 | **72**/24 | hidden | clip | `Vertical` |
| line-clamp | 160/160 | **72**/24 | hidden | clip | `Vertical` |

```
uiscan web <url> --json payload.json
uiscan clip --json payload.json        # 0=无裁切 / 1=有裁切 / 2=用法或载荷不合格
```

- 只有**可见且非空**的 DomText 进判据（`title`/`style` 这类 h=0 被排除）；`overflow:visible` 不算裁切
  （文字会画到外面）；差 1px 内按取整噪声忽略。
- **fail-closed**：载荷里没有 `sw/cw` 几何字段（旧版 `web --json`）时 `clip` **退 2** 让人重采——
  缺字段时"0 条"是"没在看"，不是"干净"。
- 真实站点跑了一把：`qingjian.app` ⇒ 截断 0、rc=0（未见误报）。
- 边界：绝对定位/居中被祖先裁掉的、以及 Canvas 内绘制的文字，这条判据看不到。

### `scan` 已把截断并进来（2026-10-20）

`scan --surface web` 现在**一次采集出两份判据**：先报道具（缺字），再报布局（截断），
共享同一次浏览器会话、同一个载荷；任一非空即 **退 1**。

真跑对照：
- `qingjian.app` ⇒ `缺字=0`、`截断=0`、rc=0；
- 本地 `truncation.html` ⇒ `缺字=0`、`截断=4`、rc=1（`p.clip/p.ell/p.vclip/p.clamp` 逐条点名）。

TUI/GUI 面目前不跑截断：TUI 的"被裁"没有独立几何可对账（格宽就是最终宽度），
GUI 需要 `TextPattern.GetBoundingRectangles` 才有得比——留作下一格。
