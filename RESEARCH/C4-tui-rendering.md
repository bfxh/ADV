# C4 深挖：TUI/终端渲染（面向 ADV CLI 交互面）

- 检索窗口：2026-10-03；优先 2025–2026 资料；锚点以链接标注，无法锚定的数字标"待本地复测"。
- ADV 前提：Windows 优先（WT/conhost/CI 三环境都要活）、本地优先、吞吐与延迟优先于花哨渲染；Rust 主体。
- 本文只覆盖 TUI/终端渲染域；不写说不清机制的条目。

---

## 0. 结论速览

1. **核心栈选 ratatui 0.30.x + crossterm 0.29.x（经 ratatui-crossterm 后端 crate）**：即时模式绘制 + 双缓冲 diff 只写变更单元，是当前 Rust TUI 事实标准（ratatui.rs highlights v030，检索日 2026-10-03）。
2. **渲染预算模型**（①，详见 §6.1）：帧成本 = cassowary 布局求解 + widget 画入 Buffer + 前后帧 diff + 只写变更单元的 I/O。ADV 判据：进度 UI ≤10–20Hz、按键到首帧 ≤50ms、draw 调用本身 ≤16ms（设计值，非外部实测）。
3. **Windows 坑清单**（②，详见 §6.2）：VT 处理需显式启用（Win10 1511 起）、conhost truecolor 仅 2016-09 后、raw mode 双平台行为不一致（crossterm #250）、IME 组合输入、CI 无 TTY。检测法：`IsTerminal` + `WT_SESSION`/`TERM_PROGRAM`/`CI` 探针。
4. **三态输出**（③，详见 §6.3）：交互 TUI（备用屏）/ 人类可读（进度条上 stderr、结果走 stdout）/ 机器 JSONL（无 ANSI 无进度）。优先级 `--json` > 非 TTY > TTY。
5. **进度+日志混流**（④，详见 §6.4）：indicatif 默认 stderr 20Hz 且非 TTY 自动隐藏（docs.rs ProgressDrawTarget），日志用 `MultiProgress::println`/`suspend` 或 tracing-indicatif 层，TUI 模式下日志落文件。
6. **依赖账**（⑤，详见 §6.5）：ratatui 直接依赖 ~10 个（cassowary/compact_str/unicode-width/bitflags/strum/indoc/itertools 等），加 crossterm+indicatif 全树预估 30–60 包；相对"零依赖"原则是**有界让步**，需 `cargo tree` 落账。

---

## A. 核心栈

### A1. ratatui（框架本体）
- **定位**：Rust TUI 事实标准；即时模式——每帧由你重画全部 widget，库负责 diff 后最小化 I/O。
- **可抄机制**：
  1. `Terminal::draw(closure)`：布局 → widget 画入 `Buffer` → 与上一帧 diff → 仅变更单元经 backend 写出 → 交换双缓冲（ratatui 官方 architecture 文档）。
  2. `Widget`/`StatefulWidget` trait：`render(area, buf)`；ADV 自定义面板（扫描结果浏览、修复确认列表）按此实现，无 retained 树。
  3. `Buffer::set_style`/cell 级 `skip` 标志：局部不重绘区域。
- **档位**：吸收。
- **第一步动作**：ADV CLI 交互面用 ratatui 0.30.x（经 ratatui-crossterm）建骨架：`ratatui::init()`（0.30 新增简化入口）+ 一个 List 面板 + mpsc 驱动的重绘。
- **许可证/成熟度**：MIT；维护中（活跃，2025 末发布 0.30 大重构）。
- **链接**：https://ratatui.rs/highlights/v030/ ；https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md

### A2. ratatui 双缓冲/diff 渲染（机制级）
- **定位**：性能核心——两个 `Buffer`（当前/上一帧），帧末 `diff()` 产出 `(x, y, &Cell)` 序列，backend 只写差异单元。
- **可抄机制**：
  1. diff 粒度是"单元"而非"行"：扫描进度数字跳变只写那一格，I/O 量与屏幕变化面积成正比——这是 ADV 进度面板低成本的关键机制。
  2. 0.30.1 增强了 buffer diff 选项（ratatui.rs highlights v030.1）。
  3. 全量重绘只发生在 resize/进入备用屏：把 resize 当作"丢弃上一帧缓冲"处理。
- **档位**：吸收。
- **第一步动作**：ADV 渲染层禁止绕过 Terminal 直写 stdout（TUI 模式）；写进度面板时优先改 cell 而非重排布局。
- **许可证/成熟度**：随 ratatui（MIT；维护中）。
- **链接**：https://ratatui.rs/concepts/rendering/ (render 流程) ；https://ratatui.rs/highlights/v0301/

### A3. ratatui 布局系统（cassowary / Flex）
- **定位**：约束求解布局——用 cassowary 线性约束求解器把 `Constraint::*`（Length/Min/Percentage/…）解成矩形集合；0.26 起 `Flex`（Legacy/SpaceAround/…）。
- **可抄机制**：
  1. 布局结果对同一终端尺寸是确定的：可在测试里对"固定宽高 → 矩形切分"做快照测试（ADV 布局回归的判据）。
  2. `Constraint::Min` 为主、`Length` 为辅的切法适合"固定头部 + 弹性列表 + 状态条"。
  3. 求解成本随区域数线性量级（几十个区域 µs 级——量级判断，待本地 bench 复核），ADV 不需要在每帧缓存布局之外做额外优化。
- **档位**：吸收（随 ratatui）。
- **第一步动作**：主面板三段切法（头部提示/结果列表/底部键位条）定约束写死，进入快照测试。
- **许可证/成熟度**：cassowary crate MIT/Apache-2.0（以 crates.io 为准）；维护中。
- **链接**：https://docs.rs/ratatui/latest/ratatui/layout/index.html

### A4. ratatui 0.30 crate 拆分（ratatui-core / ratatui-widgets / ratatui-crossterm）
- **定位**：2025 末的模块化重构——core（trait+layout）/widgets（控件库）/backend crate（如 ratatui-crossterm）分离；主 crate 依赖升到 crossterm 0.29。
- **可抄机制**：
  1. 依赖裁剪面：只要 layout+自绘 widget 时可只依赖 `ratatui-core`（ADV 若不走完整控件库是省依赖的口子）。
  2. backend 独立演进：换 backend（如未来 termwiz）不动业务代码。
  3. feature flag 选择 backend（ratatui.rs feature-flags 文档）。
- **档位**：吸收。
- **第一步动作**：Cargo.toml 固定 `ratatui = "0.30"` 并记录启用 feature；`cargo tree` 落账（见 §6.5）。
- **许可证/成熟度**：MIT；维护中（2026 仍在发 0.30.x 补丁）。
- **链接**：https://ratatui.rs/highlights/v030/ ；https://ratatui.rs/installation/feature-flags/

### A5. ratatui 事件循环 / 帧节流模式
- **定位**：社区标准写法——`event::poll(timeout)` 阻塞上限 + 有事件或 tick 才 `draw`；tick 与 frame 分离。
- **可抄机制**：
  1. `poll(Duration::from_millis(50~100))`：无输入时睡眠，事件到达立即返回——天然的事件驱动节流，不是忙等。
  2. "重绘仅当状态变化"：后台扫描线程经 mpsc 推进度事件，UI 线程收到的 batch 合并后一次 draw（避免每条 finding 一帧）。
  3. panic hook 恢复终端（0.30 的 `ratatui::init()` 内建 restore hook）——Windows 终端崩进花屏是最高频差评源，必须接管。
- **档位**：吸收（作为 ADV 事件循环骨架）。
- **第一步动作**：ADV `tui.rs`：`init()` + mpsc + `poll(50ms)` + 变脏才 draw；panic hook 落文件日志。
- **许可证/成熟度**：模式来源 ratatui 官方示例/文档；MIT。
- **链接**：https://ratatui.rs/tutorials/ （事件循环示例） ；https://docs.rs/ratatui/latest/ratatui/#getting-started

### A6. crossterm（跨平台终端控制）
- **定位**：ratatui 默认后端；raw mode、备用屏、事件、样式命令的跨平台抽象——Unix 走 ANSI/tty，Windows 走 Console API（windows-sys/WinAPI）。
- **可抄机制**：
  1. `execute!`/`queue!` 命令模式：命令批量入队一次 flush——ADV 高频进度刷新走 queue 合并写。
  2. `EnterAlternateScreen`/`EnableMouseCapture`/`EnableBracketedPaste` 等命令化开关，进出可逆（ADV 退出路径必须逐项还原）。
  3. `crossterm::style::Stylize` 非交互模式下的着色输出可复用同一依赖（见 §C 探针）。
- **档位**：吸收。
- **第一步动作**：随 ratatui-crossterm 引入；封装 `TerminalGuard`（RAII：进备用屏/raw，Drop 还原）。
- **许可证/成熟度**：MIT；维护中——2025 起维护归入 ratatui 组织（原 crossterm-rs 由原作者 Timon Post 维护，现与 ratatui 工作区耦合，backend crate `ratatui-crossterm` 已入 ratatui workspace）。
- **链接**：https://docs.rs/crossterm/latest/crossterm/ ；https://github.com/ratatui/ratatui

### A7. crossterm Windows 后端（VT/WinAPI 双轨）
- **定位**：Windows 上 crossterm 的输出/输入分两条路：输出经 `ENABLE_VIRTUAL_TERMINAL_PROCESSING` 后发 ANSI；输入默认走 Console API 读 `INPUT_RECORD`（非 VT 字节流）。
- **可抄机制**：
  1. raw mode 的 Windows 语义 = 关 `ENABLE_LINE_INPUT`/`ENABLE_ECHO_INPUT`/`ENABLE_PROCESSED_INPUT`（crossterm terminal 文档 + issue #250 明确双平台不一致）——ADV 判据：不要假设 Unix 行为（`\r\n`、信号）在 Windows 成立。
  2. VT 处理标志启用失败（老 conhost）时 crossterm 仍走 Console API 完成基本功能——但 ADV 需自检测后降级着色（§6.2）。
  3. Windows 下 `\n` 能动但不回车、伴随闪烁（crossterm #584 实测记录）——ADV 全部输出用 `\r\n`（TUI 内由库处理，非 TUI 人类模式自己保证）。
- **档位**：吸收。
- **第一步动作**：Windows smoke 脚本：WT、conhost、GHA 三环境各跑一遍 ADV `--version`/TUI 进退/颜色三态。
- **许可证/成熟度**：随 crossterm（MIT；维护中）。
- **链接**：https://github.com/crossterm-rs/crossterm/issues/250 ；https://github.com/crossterm-rs/crossterm/issues/584 ；https://docs.rs/crossterm/latest/crossterm/terminal/index.html

### A8. crossterm kitty 键盘协议（KeyboardEnhancementFlags）
- **定位**：增强键事件协议（区分左右修饰键、键释放、组合消歧）；crossterm 提供 `PushKeyboardEnhancementFlags`/`PopKeyboardEnhancementFlags` + `supports_keyboard_enhancement()` 探测。
- **可抄机制**：
  1. 能力探测后再启用：不支持的终端（含 Windows Console）push 无效但静默——ADV 用探测结果决定是否启用"按住松开"类交互（修复确认里的多键导航不需要）。
  2. 2026-05 社区 PR 加入 opt-in Kitty `REPORT_ALL_KEYS`（github）——协议覆盖在持续演进，ADV 只用稳定子集（DISAMBIGUATE_ESCAPE_CODES 级别）。
- **档位**：有界（Windows 优先场景收益低；WT 原生不走 kitty 协议）。
- **第一步动作**：暂不启用；在终端能力探测函数里留 `supports_keyboard_enhancement()` 位。
- **许可证/成熟度**：随 crossterm（MIT；维护中）。
- **链接**：https://docs.rs/crossterm/latest/crossterm/event/struct.PushKeyboardEnhancementFlags.html

### A9. termion（对照）
- **定位**：Unix-only 极简 tty 库（libc + 直接写 ANSI），ratatui 有 TermionBackend。
- **机制**：无 Windows 后端——对 Windows 优先的 ADV 是硬伤，无机制可抄。
- **档位**：不吸收。
- **第一步动作**：无。
- **许可证/成熟度**：MIT；维护中（低频）。
- **链接**：https://crates.io/crates/termion

### A10. tui-rs（对照，弃维护）
- **定位**：ratatui 的前身；2022 起社区分叉到 ratatui，原仓归档。
- **机制**：与 ratatui 同源（Buffer/diff/后端抽象均继承自它），无增量可抄。
- **档位**：不吸收。
- **第一步动作**：禁止引入 `tui-rs` 依赖（审计规则可加：deny tui 0.19.x）。
- **许可证/成熟度**：MIT；弃维护（archived）。
- **链接**：https://github.com/fdehau/tui-rs （archived）

---

## B. 进度 / 输出 / 着色

### B11. indicatif（进度条）
- **定位**：Rust 进度条事实标准；`ProgressBar`/`MultiProgress` + 模板样式 + 节流绘制。
- **可抄机制**：
  1. **默认绘制目标 = 缓冲 stderr，最高 20 次/秒**（docs.rs ProgressDrawTarget 原文）；`MultiProgress` 默认 15fps——节流是内建的，ADV 不用自己写帧率闸。
  2. **非 TTY 自动隐藏**：stderr 不是终端时 bar 默认不渲染（issue #87 的行为基线；`stderr_nohide()`/`stderr_with_hz()` 可覆盖）——正好命中 ADV"管道环境降级"要求，默认行为即正确。
  3. `ProgressBar::println`：在 bar 上方打印一行日志而不破坏 bar；**bar 隐藏时 println 也随之不输出**（docs.rs 已知 gotcha）——机器模式必须绕开它直接写 stdout。
- **档位**：吸收。
- **第一步动作**：扫描/索引任务统一走 `MultiProgress`；进度事件由工作线程 `inc_message`/`set_position`，UI 线程不轮询。
- **许可证/成熟度**：MIT；维护中（console-rs 组织）。
- **链接**：https://docs.rs/indicatif/latest/indicatif/struct.ProgressDrawTarget.html ；https://github.com/console-rs/indicatif/issues/87

### B12. console crate
- **定位**：indicatif 的底座；Term 抽象、`colors_supported`、TTY/宽高检测、Windows 控制台 API。
- **可抄机制**：
  1. `Term::stderr()` + `features().colors_supported()`：能力探测（含 NO_COLOR/CLICOLOR 约定处理）——ADV 着色决策的参考实现。
  2. `Term::write_line`：整行原子写，多线程输出不撕行。
- **档位**：有界（作为 indicatif 传递依赖自动在场；不直接依赖）。
- **第一步动作**：不显式引入；能力探测优先用 std + 环境探针（§B17）。
- **许可证/成熟度**：MIT/Apache-2.0；维护中。
- **链接**：https://docs.rs/console/latest/console/

### B13. owo-colors（着色）
- **定位**：零开销着色库；`if_supports_color` 经 supports-color 探测。
- **可抄机制**：style wrapper 按需包裹字符串、支持 `Stream` 级探测——人类可读模式的着色方案候选。
- **档位**：有界（与 anstyle 二选一，见 B15）。
- **第一步动作**：若 ADV 用 clap，优先 anstyle 生态（B15）保持一致；owo-colors 作备选记录。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://crates.io/crates/owo-colors

### B14. colored（对照）
- **定位**：全局默认色开关的简易着色库。
- **机制**：全局可变状态（`colored::control`）——多线程/嵌套库场景互相覆盖；对 ADV 无正收益机制。
- **档位**：不吸收。
- **第一步动作**：无。
- **许可证/成熟度**：MIT；维护中（低频）。
- **链接**：https://crates.io/crates/colored

### B15. anstyle / anstream（clap 生态着色流）
- **定位**：clap 4 生态的着色/流适配标准；anstream 按目标自动开/剥 ANSI。
- **可抄机制**：
  1. `AutoStream::choice(&stream)`：把"TTTY 与否 + 环境变量"折成一个 `ColorChoice` 决策——正好是 ADV 人类模式着色管线的现成机制。
  2. anstyle-query 集中处理 NO_COLOR/CLICOLOR/CLICOLOR_FORCE/TERM=dumb 约定。
- **档位**：有界（随 clap 自动在场；ADV 自研着色统一走它，不引 owo-colors）。
- **第一步动作**：CLI 参数帮助/错误输出直接吃 clap 内建着色；人类可读结果输出包 `AutoStream`。
- **许可证/成熟度**：MIT/Apache-2.0；维护中。
- **链接**：https://docs.rs/anstream/latest/anstream/

### B16. ANSI 转义工程（SGR / OSC 8 / NO_COLOR）
- **定位**：底层协议面——SGR 颜色/粗体、OSC 8 超链接、无色约定。
- **可抄机制**：
  1. OSC 8 超链接（`ESC]8;;URL ESC\` 文本 `ESC]8;; ESC\`）：ADV 在人类模式把 finding 行直接链到文件路径/规则页——WT、kitty、wezterm 均已支持，conhost 不支持也不报错（降级安全）。
  2. NO_COLOR / CLICOLOR_FORCE / TERM=dumb 约定（no-color.org 标准）作为着色决策输入，优先级写进 §6.3 三态矩阵。
  3. SGR 38;2 truecolor 在 conhost 需 Windows 10 2016-09 后（MS 工程师公开确认 + colortool 时代）——ADV 探测失败降 256 色。
- **档位**：reference（协议知识，无新依赖）。
- **第一步动作**：`output/color.rs` 实现颜色决策函数（输入：环境探针 + `--color` 旗标；输出：ColorChoice），配三环境冒烟。
- **许可证/成熟度**：协议标准；NO_COLOR 约定 2021 起广泛落地。
- **链接**：https://no-color.org/ ；https://learn.microsoft.com/en-us/windows/console/classic-console-apis-versus-virtual-terminal-sequences

### B17. 终端/环境探针组（std::io::IsTerminal + 环境变量）
- **定位**：三态决策的前置事实源；std 自带，零依赖。
- **可抄机制**：
  1. `std::io::IsTerminal`（Rust 1.70 稳定）对 stdout/stderr 分别探测——ADV 判据：stdout/stderr 分开判，因为"进度上 stderr、结果走 stdout"会让两者 TTY 状态不同。
  2. 环境探针：`WT_SESSION`（在 Windows Terminal 内）、`TERM_PROGRAM=vscode`、`CI`/`GITHUB_ACTIONS`/`TF_BUILD`、`TERM=dumb`、`NO_COLOR`。
  3. 探测失败安全：任何探针缺省都落到"最保守"（无色 + 人类可读）。
- **档位**：吸收。
- **第一步动作**：`env_probe.rs` 单元测试覆盖探针矩阵（表格驱动，不依赖真终端）。
- **许可证/成熟度**：std；—。
- **链接**：https://doc.rust-lang.org/std/io/trait.IsTerminal.html

---

## C. 交互输入与新锐

### C18. crossterm event 流（输入事件）
- **定位**：`event::poll`/`read` 同步接口 + `event-stream` feature 的异步 `EventStream`；键盘/鼠标/resize/paste 统一事件类型。
- **可抄机制**：
  1. 同步 poll 循环（§A5）对 ADV 够用且少一个 async 依赖面；`EventStream` 仅在 ADV 已有 tokio 运行时时考虑——ADV 判据：CLI 交互面不为此引 tokio。
  2. resize 事件在 Windows 来自控制台缓冲区尺寸轮询——高频 resize 期间需合并（去抖 ~100ms，设计值）。
  3. bracketed paste：粘贴批量 finding 路径时把整段作为一个事件——防逐键重绘。
- **档位**：吸收。
- **第一步动作**：事件循环支持 Key/Resize/Paste 三类；Mouse 暂不开。
- **许可证/成熟度**：随 crossterm；维护中。
- **链接**：https://docs.rs/crossterm/latest/crossterm/event/index.html

### C19. Windows 控制台输入模式（raw mode 代价 / IME）
- **定位**：Win32 `SetConsoleMode` 的输入侧语义：`ENABLE_VIRTUAL_TERMINAL_INPUT` 把 VT 序列直递应用；关 line/echo/processed 即 raw。
- **可抄机制**：
  1. IME 风险：VT 输入模式下组合中的中文文本通常不会以完整 `Event::Key` 呈现（MS Learn 高层控制台模式文档 + crossterm 社区已知坑）——ADV 交互面不含中文输入框，规避即可；搜索过滤框若要支持中文，判据是"用 WT/默认（WinAPI）输入路径 + 过滤框逐字符过滤"实测验证。
  2. raw mode 下 Ctrl+C 不再产生 SIGINT（Unix）/被 processed 吞掉——ADV 必须显式处理 `KeyCode::Char('c')` + Ctrl 的退出路径并还原终端。
  3. crossterm 的 WinAPI 输入路径读 UTF-16 记录并处理代理对——非 ASCII 文件名在结果列表中显示宽度依赖 unicode-width（§E28）。
- **档位**：有界（知识面 + 冒烟用例，不引新依赖）。
- **第一步动作**：Windows 冒烟清单加两条：中文文件名显示、Ctrl+C 退出后终端状态干净。
- **许可证/成熟度**：Win32 协议面；—。
- **链接**：https://learn.microsoft.com/en-us/windows/console/high-level-console-modes ；https://github.com/crossterm-rs/crossterm/issues/250

### C20. bubbletea-rs（新锐对照）
- **定位**：Charm 的 Elm 架构 Rust TUI（Model-Update-View），基于其自研 termfx/ansi 层。
- **机制**：消息驱动（Msg → Update → View 重渲）与 ratatui 即时模式同构，差别在框架强制分层；ADV 自研事件循环足够，引入整套架构框架是负资产。
- **档位**：watch（1.x 已发布，生态在涨；若 ratatui 线受阻再评估）。
- **第一步动作**：无；半年后复查一次版本与依赖树。
- **许可证/成熟度**：MIT；维护中（charmbracelet 组织）。
- **链接**：https://github.com/charmbracelet/bubbletea-rs

### C21. ratzilla（实验性）
- **定位**：ratatui + WASM 跑浏览器，Orhun 出品，2025-01-31 首发 crates.io，已入 ratatui 组织；受益于 ratatui 2025 的 no_std 化。
- **机制**：DOM/canvas 作 backend——证明 ratatui 的 Buffer/diff 层可移植到任意渲染面；对 ADV 本地 CLI 无直接用途。
- **档位**：watch。
- **第一步动作**：无（潜在远期：ADV Web 报告页复用 TUI 视觉语言）。
- **许可证/成熟度**：MIT；实验性但活跃（2025-04 GitHub Trending #4，trendshift）。
- **链接**：https://github.com/ratatui/ratzilla

---

## D. 终端模拟器（渲染侧对照 / 兼容目标）

### D22. Windows Terminal + conhost/OpenConsole
- **定位**：ADV Windows 的一等公民目标。WT：全 VT/truecolor、Atlas 文本渲染、默认主机；conhost：旧主机，VT 支持随 Windows 版本渐进（`ENABLE_VIRTUAL_TERMINAL_PROCESSING` 自 Win10 1511、truecolor 自 2016-09、近期 build 默认开 VT）。
- **可抄机制（兼容坑清单见 §6.2）**：
  1. WT 检测 = `WT_SESSION` 存在；conhost = Windows 且无该变量 → 渲染档位降一档。
  2. conhost 多线程交错写有历史问题（同步句柄语义），WT/OpenConsole 已修——ADV 判据：非 TUI 模式输出走单 writer 线程 + 消息队列，不依赖宿主修复。
  3. WT 默认 UTF-8 正常；conhost 代码页可能是 OEM——ADV 启动时 `SetConsoleOutputCP(CP_UTF8)`（winapi/windows-sys 一次调用），失败则 ASCII 化输出（待本地复测项）。
- **档位**：reference。
- **第一步动作**：§6.2 坑清单写进 ADV `docs/terminal-matrix.md` 并配 CI 冒烟（GHA windows-latest 覆盖 conhost-like 环境）。
- **许可证/成熟度**：WT MIT（开源 2019，GA 2020）；—。
- **链接**：https://learn.microsoft.com/en-us/windows/console/classic-console-apis-versus-virtual-terminal-sequences ；https://github.com/microsoft/terminal

### D23. alacritty（对照）
- **定位**：GPU（OpenGL）网格渲染器、无标签页无图像协议——性能上限参照系。
- **机制**：字形图集 + 网格 damage 重绘，终端侧每帧成本极低 ⇒ ADV 的瓶颈大概率在自己进程（布局求解/写 I/O）而非模拟器。
- **档位**：reference。
- **第一步动作**：无。
- **许可证/成熟度**：Apache-2.0；维护中。
- **链接**：https://github.com/alacritty/alacritty

### D24. kitty（对照）
- **定位**：图像协议（APNG 经 GPU）+ 键盘增强协议发源地。
- **机制**：图像协议对 ADV 安全扫描输出无刚需（无截图渲染需求）；键盘协议经 crossterm 有界接入（§A8）。
- **档位**：reference。
- **第一步动作**：无。
- **许可证/成熟度**：GPL-3.0（BSD-3 三方库）；维护中。
- **链接**：https://sw.kovidgoyal.net/kitty/

### D25. wezterm（对照）
- **定位**：GPU + 连接复用器 + ligature + 多图像协议；Windows 支持好。
- **机制**：作为 ADV 兼容目标之一（`TERM_PROGRAM=WezTerm` 可探测）；其内嵌 termwiz TUI 库证明"库即终端"路线可行，但依赖面大，不吸收。
- **档位**：reference。
- **第一步动作**：冒烟矩阵加一行。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://github.com/wez/wezterm

---

## E. 工程方案

### E26. 进度 + 日志混流（tracing-indicatif / println / suspend）
- **定位**：进度条与结构化日志同流不打架的三个层级方案。
- **可抄机制**：
  1. 流分离（第一层）：进度条走 stderr（indicatif 默认），结果/机器输出走 stdout——管道下 `2>`丢弃进度即可得干净结果流。
  2. `MultiProgress::println` / `suspend()`（第二层）：日志行经 bar 的 println 打在 bar 上方；直接写必须包 suspend（MultiProgress 只持弱引用，docs.rs）。
  3. tracing-indicatif（第三层）：tracing layer 按 span 自动建 bar 并防互踩，indicatif README 推荐的组合方式——ADV 若全面上 tracing 则接它，否则第二层已够。
  4. TUI 模式例外：备用屏期间日志必须落文件（tracing-appender），绝不写 stderr。
- **档位**：吸收。
- **第一步动作**：`--verbose` 时日志入 `~/.adv/logs/<ts>.log` + stderr 只留进度条；实现 `ProgressLogBridge`（封装 MultiProgress::println）。
- **许可证/成熟度**：tracing-indicatif MIT，维护中；indicatif MIT 维护中。
- **链接**：https://docs.rs/tracing-indicatif/latest/tracing_indicatif/ ；https://docs.rs/indicatif/latest/indicatif/struct.MultiProgress.html

### E27. 非 TTY 三态降级设计（ADV 输出契约）
- **定位**：ADV CLI 输出三态：交互 TUI / 人类可读 / 机器 JSONL；决策矩阵见 §6.3。
- **可抄机制**：
  1. 判定顺序固定：显式旗标（`--json`/`--ui`）> TTY 探测 > 默认人类可读——显式永远赢，保证脚本可预测。
  2. indicatif 非 TTY 自动隐藏 + anstream 自动剥 ANSI ⇒ 人类模式在管道里天然退化成纯文本（默认行为即契约，零额外代码）。
  3. JSONL 态硬约束：无 ANSI、无进度、每行一个自包含 JSON 对象（finding/summary 两种事件类型）；`--json` 下 `ProgressBar::println` 的"隐藏即不输出"gotcha 恰好保证不污染 stdout（docs.rs 行为锚）。
- **档位**：吸收（ADV 自有设计，机制来源如上）。
- **第一步动作**：`OutputMode` 枚举 + 决策函数 + 三态快照测试（同一 findings 输入 × 三态 golden 文件）。
- **许可证/成熟度**：—。
- **链接**：https://docs.rs/indicatif/latest/indicatif/struct.ProgressBar.html （println gotcha）

### E28. TUI 栈依赖成本账
- **定位**：与"零依赖"原则的权衡记录。
- **机制/账目**：
  1. ratatui 0.30 直接依赖（crates.io/docs.rs 页面为准）：cassowary、compact_str、unicode-width、unicode-segmentation、bitflags、strum、indoc、itertools、（可选）crossterm/ratatui-crossterm、serde——纯渲染件无重运行时。
  2. crossterm 侧：windows-sys（Windows）/nix+libc+signal-hook（Unix）/mio（event-stream 可关）。
  3. indicatif：console + number-prefix 为主。
  4. 全树预估 30–60 包（估算，待 ADV 落地后 `cargo tree -e no-dev` 复核落账）；`event-stream` feature 关闭可少带 futures/mio。
- **档位**：有界（允许依赖，账必须落）。
- **第一步动作**：ADV 引入后 `cargo tree -e no-dev | wc -l` 与 `cargo deny` 结果写进依赖账文件；基线外新增需走 PR 说明。
- **许可证/成熟度**：各件 MIT / MIT-Apache 双许可（以 crates.io 为准）。
- **链接**：https://docs.rs/crate/ratatui/latest/ （Dependencies 标签页）

---

## 6. 五个重点问题的答案

### 6.1 ① ratatui+crossterm 渲染预算模型（给 ADV 交互面的判据）

```
一帧 = poll(事件, timeout) ─┐
                            ├→ 状态变化? → draw:
后台线程 mpsc ─────────────┘     ① 布局求解（cassowary，区域数线性量级，µs 级*
                                  ② widgets 画入 Buffer（纯内存，面积线性）
                                  ③ 与上一帧 diff → 只写变更单元（crossterm queue 批量）
                                  ④ flush 一次
```
- **机制锚**：③ 由 ratatui Buffer diff 保证——I/O 量 ∝ 屏幕变化面积而非全屏（ratatui 渲染文档）。
- **ADV 判据（设计值，落地后 bench 复核）**：
  - 进度面板重绘频率 ≤10–20Hz（与 indicatif 默认 20Hz 同量级，docs.rs 锚）；
  - 按键→首帧可见 ≤50ms（感知即时阈值，设计值）；
  - 单次 `draw` 自耗时 ≤16ms（60fps 上限；交互面实际 20–30fps 已流畅——设计值）；
  - 扫描工作线程永不直接碰终端，只发事件；UI 线程合并 batch 后一帧；
  - resize 去抖 ~100ms 后全量重绘（crossterm Windows resize 来自轮询，事件可能连发）。
- *µs 级为量级推断（cassowary 求解几十约束），非实测锚——ADV 落地后用 `cargo bench` 复核。

### 6.2 ② Windows 终端兼容坑清单与检测法

| # | 坑 | 触发条件 | 检测/规避 |
|---|---|---|---|
| 1 | ANSI 原样打印 | `ENABLE_VIRTUAL_TERMINAL_PROCESSING` 未启用（Win10 1511 前无此旗标；MS Learn） | SetConsoleMode 返回值失败 ⇒ 关色；crossterm 输出侧已代管，自研直写路径需自查 |
| 2 | truecolor 缺失 | 老 conhost（truecolor 自 2016-09，MS 公开确认） | `WT_SESSION` 存在 ⇒ 满配；否则降 256 色/16 色 |
| 3 | 中文乱码 | conhost 代码页非 UTF-8 | 启动 `SetConsoleOutputCP(CP_UTF8)`；失败 ⇒ ASCII 化（待本地复测） |
| 4 | raw mode 双平台不一致 | crossterm #250：Windows 关三个 ENABLE_* 旗标实现，`\n` 不回车且闪烁（#584） | 非 TUI 人类模式统一 `\r\n`；TUI 交给库 |
| 5 | IME 组合输入丢失 | `ENABLE_VIRTUAL_TERMINAL_INPUT` 开启时组合文本不进 Event::Key | ADV 交互面无输入框即规避；要中文过滤框先在 WT 实测 |
| 6 | Ctrl+C 不退 | raw mode 无 processed input | 显式处理 Ctrl+C + panic hook + Drop 还原 |
| 7 | CI 无 TTY | GITHUB_ACTIONS/TF_BUILD 等环境 stdout/stderr 重定向 | `IsTerminal`=false ⇒ 强制非 TUI 态；GHA 日志支持部分 ANSI 但按无色设计 |
| 8 | 多线程输出交错 | conhost 历史同步句柄问题，WT 已修 | 单 writer 线程 + 队列，不赌宿主 |
| 9 | resize 连发 | Windows resize 事件来自缓冲区尺寸轮询 | 去抖 ~100ms |

### 6.3 ③ 三态降级决策矩阵

| 条件（按序判定，先命中先用） | 输出态 | 着色 | 进度 | 结果流 |
|---|---|---|---|---|
| `--json` | JSONL | 无 | 无（bar 隐藏 + 不用 println） | stdout，逐行 JSON |
| `--ui` 且 stdout 或 stdin TTY | 交互 TUI | 全 | 备用屏内自绘 | 退出时摘要到 stdout |
| 非 TTY（管道/CI/重定向） | 人类可读 | 无（anstream 剥除） | indicatif 自动隐藏 | stdout 纯文本；日志 stderr |
| TTY（默认） | 人类可读 | 按 NO_COLOR/CLICOLOR/`--color` | stderr 进度条 | stdout；日志 stderr/文件 |
- 着色优先级：`--color=never` > NO_COLOR > TTY 探测 > CLICOLOR_FORCE（约定见 no-color.org）。
- 机制锚：indicatif 非 TTY 自动隐藏（docs.rs）、anstream AutoStream 自动剥 ANSI、`IsTerminal` std 稳定——三态的降级路径大部分是"库默认行为"，ADV 只需把判定顺序写死并测试。

### 6.4 ④ 进度与日志混流
- 三层递进（§E26）：流分离 → `MultiProgress::println`/`suspend` → tracing-indicatif 层；TUI 态日志一律落文件。
- ADV 落法：默认只有进度条（stderr）；`-v` 起文件日志 + 一行进度摘要；`-vv` 才把日志桥到 stderr（经 println 桥）。

### 6.5 ⑤ 依赖成本账
- 见 §E28：ratatui 直接依赖 ~10 件、加 crossterm/indicatif 全树预估 30–60 包（估算，待 `cargo tree` 落账）；可控项：关 crossterm `event-stream`（免 mio/futures）、不引 tokio 不引 async 全家桶。
- 与零依赖原则的关系：终端 TUI 无"零依赖实现"的现实路径（Windows VT/WinAPI 双轨 + diff 渲染自研成本高且易错）；让步以"账可审计"为界——基线哈希外新增依赖走 PR 说明。

---

## 7. 2025–2026 前沿信号

1. **ratatui 0.30 模块化**（2025 末）：core/widgets/backend 拆分 + `ratatui::init()` 简化入口 + crossterm 0.29；0.30.1 继续增强 diff 选项（ratatui.rs highlights）。
2. **crossterm 维护归入 ratatui 组织**：backend 固化为 `ratatui-crossterm` crate，单一事实源（github ratatui org）。
3. **ratatui no_std 化**（2025）：Buffer/Style 层脱离 std，打开 WASM/嵌入式面（Orhun 2025 年终文）；ratzilla 据此跑浏览器。
4. **kitty 键盘协议持续铺开**：crossterm 0.29 稳定 push/pop + 探测；2026-05 社区 PR 加 REPORT_ALL_KEYS（github）。WT 原生不支持——Windows 优先项目收益有限。
5. **新框架层薄化**：bubbletea-rs（Elm 架构）1.x；ratzilla 等实验——共同点是"更薄的后端抽象 + 可移植渲染核"，与 ADV 相关的只有"Buffer/diff 层可复用"这一点。
6. **着色约定收敛**：NO_COLOR/CLICOLOR_FORCE 成为默认约定（no-color.org），anstream/anstyle-query 处处实现——ADV 直接消费约定，不自研。

## Top-3（对 ADV 的优先级）

1. **吸收 ratatui 0.30 + crossterm 0.29 栈，把渲染预算模型写进 ADV 交互面骨架**（事件驱动 + 变脏才画 + mpsc 合并 + panic hook）——这是交互体验与吞吐的最小机制集（§A1–A5、§6.1）。
2. **三态输出契约先行**（`--json` > 非 TTY > TTY + 着色约定矩阵）：indicatif/anstream/std 探针的默认行为已覆盖 80% 降级路径，先写死判定顺序与 golden 测试，再谈 TUI（§6.3、§B17、§E27）。
3. **Windows 终端矩阵 + 冒烟脚本**：§6.2 九坑清单 → `docs/terminal-matrix.md` + WT/conhost/GHA 三环境自动冒烟，纳入 CI 门（§D22、§C19）。

---

### 附：条目速查（档位 × 许可证 × 成熟度）

| 条目 | 档位 | 许可证 | 成熟度 |
|---|---|---|---|
| ratatui（含布局/diff/0.30 拆分/事件循环） | 吸收 | MIT | 维护中（活跃） |
| crossterm（含 Windows 后端/kitty 协议） | 吸收/有界 | MIT | 维护中（ratatui org） |
| indicatif / console / tracing-indicatif | 吸收/有界 | MIT / MIT-Apache | 维护中 |
| anstyle/anstream（随 clap） | 有界 | MIT-Apache | 维护中 |
| owo-colors | 有界（备选） | MIT | 维护中 |
| colored | 不吸收 | MIT | 维护中（低频） |
| termion / tui-rs | 不吸收 | MIT | 低频 / 弃维护 |
| bubbletea-rs / ratzilla | watch | MIT | 维护中 / 实验性活跃 |
| ANSI 工程 / 探针组 / 三态设计 / 混流方案 | reference/吸收 | —/std/MIT | — |
| WT+conhost / alacritty / kitty / wezterm | reference | MIT / Apache-2.0 / GPL-3.0 / MIT | 维护中 |
| 依赖成本账 | 有界 | 多件 MIT 族 | 以 crates.io 为准 |
