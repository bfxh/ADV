# 实验性开发者工具集成分析

> 12 个非智能体开发者工具的调研与 ADV 集成可行性评估
> 调研日期：2026-10-02

---

## 总览

| # | 工具 | 生态 | 安装 | 语言覆盖 | 成熟度 | ADV 适配度 |
|---|------|------|------|----------|--------|------------|
| 1 | **splice** | cargo | `cargo install splice` | Rust/Python/C/C++/Java/JS/TS (7) | v3.0，有 Magellan 索引依赖 | ★★★ 高 |
| 2 | **refute** | go | `go install ...refute@latest` | Go(主)/Rust(实验)/TS(实验) | v0.1 dogfood | ★★☆ 中 |
| 3 | **Codexray** | npm | `npm i -g codexray` | TypeScript only | 有 10 条规则 | ★★☆ 中 |
| 4 | **Typewise** | npm | `npm i -g @rcsv/typewise` | TypeScript only | v0.1.1 | ★☆☆ 低 |
| 5 | **Vigilo** | pip | `pip install vigilo` | Python/JS/TS | v0.3.4 | ★★★ 高 |
| 6 | **Metrolith** | pip | `pip install metrolith` | Python/Java/JS/TS/Go | 需 3.13 | ★★☆ 中 |
| 7 | **Skylos** | pip | `pip install skylos` | 多语言 | 功能丰富 | ★★★ 高 |
| 8 | **Reality** | npm | `npm i reality-code-review` | React/Next.js only | v0.1.15 | ★☆☆ 低 |
| 9 | **DiffSense** | npm | `npm i -g @phoenixaihub/diffsense` | JS/TS/Python | v1.0.0 | ★★☆ 中 |
| 10 | **pyvivo** | pip | `pip install pyvivo` | Python (需 3.14+) | v0.1.0 研究原型 | ★☆☆ 低 |
| 11 | **propre** | pip | `pip install propre-cli` | Python | v0.1.1，文档极少 | ★☆☆ 低 |
| 12 | **Fallow** | npm/cargo | `npm i -D fallow` | TypeScript/JavaScript | v3，有 GH Action | ★★★ 高 |

---

## 第一梯队：可直接接入 ADV 门链

### 1. splice — 跨文件安全重构内核

**核心价值**：字节级精确替换 + tree-sitter 语法验证 + 编译器校验，失败自动回滚。对 ADV 的 `rename`/`edit` 场景是天然护栏。

**集成方案**：
- 前置：需先跑 `magellan watch --root ./src --db .magellan/splice.db` 建索引
- 门链接入：在 `local_gate.py` 的 STEPS 里加一步 `splice status` 检查索引健康
- 与现有 LSP 架构的关系：splice 走 Magellan 图数据库，和 LSP 互补（LSP 做实时补全，splice 做批量重构）
- 风险：Magellan 是额外守护进程，需评估资源开销

**适配点**：可替代 `tools/lsp.py` 中部分 rename 场景，提供验证后再写入的安全网。

### 2. Vigilo — 零配置安全扫描

**核心价值**：AST + 数据流分析，检测第一方代码漏洞，低误报。ADV 已有 `spec/VULN-HUNTING.md` 和 `spec/SCAN-POLICY.md`，Vigilo 可作为安全门。

**集成方案**：
- 零配置直接跑：`vigilo ./src`
- 门链接入：作为 SAST 步骤加进 CI（`core.yml`），exit code 驱动 pass/fail
- 与现有 `bug_scan` 的关系：bug_scan 偏功能正确性，Vigilo 偏安全漏洞，正交
- 语言覆盖：Python/JS/TS 正好覆盖 ADV 主要代码

**适配点**：直接作为安全门步骤，输出 JSON 可喂给现有报告管线。

### 3. Skylos — 预合并全功能扫描器

**核心价值**：死代码检测 + 安全 bug + 密钥泄露 + AI 生成代码错误检测 + SBOM 导出。功能最全面。

**集成方案**：
- `skylos pre-commit` 可接 pre-commit hook
- `skylos cicd init` 生成 CI 工作流
- `skylos verify` 专检 AI 幻觉——与 ADV 的 `hallucination_guard` 互补
- `skylos defend` 验证 agent 护栏
- SBOM 导出（CycloneDX/SPDX）可用于依赖审计

**适配点**：一个工具覆盖多个现有需求（死代码、密钥泄露、AI 幻觉），ROI 最高。

### 4. Fallow — TS/JS 代码库健康度

**核心价值**：统一图分析复杂度/重复/死代码/架构。有 GitHub Action (`fallow-rs/fallow@v3`)、VS Code 扩展、LSP、agent hook 四种接入面。

**集成方案**：
- `npx fallow audit` 只检查增量变更——适合 CI 增量门
- `fallow health` 输出健康度评分可做 god-ratchet 新轴
- `npx fallow agent install` 直接接 agent 护栏
- GH Action 一行接入：`uses: fallow-rs/fallow@v3`

**适配点**：`audit` 模式的增量检查与 ADV 的"只准减不准增"纪律天然对齐。

---

## 第二梯队：有条件可用

### 5. refute — LSP 驱动重构

**核心价值**：用语言服务器做 IDE 级重构（rename/extract-function/inline/change-signature）。

**限制**：v0.1 dogfood 阶段；Go 为主，Rust/TS 实验性；每次调用启动新 LS 进程。

**集成方案**：
- `refute doctor` 检查 LS 可用性
- `refute rename --dry-run --json` 可预览变更
- 与 ADV 现有 `tools/lsp.py` 架构重叠度高——如果接入，应作为 lsp.py 的下游消费者而非独立工具

**前置条件**：等 Rust 支持脱离 experimental。

### 6. Metrolith — 多语言度量

**核心价值**：版本化代码度量 + 离线报告。

**限制**：需 Python 3.13（ADV 本机 3.14 裸装、3.11 有 numpy）；文档较少。

**集成方案**：
- `metrolith validate` 可设阈值门
- 与 ADV 现有 god-baseline 机制可能重叠

**前置条件**：确认 Python 版本兼容性；评估与 god-baseline 的度量是否冗余。

### 7. DiffSense — 语义 diff

**核心价值**：GumTree AST diff 区分结构变更 vs 格式变更，数据流分析给风险评分。

**限制**：v1.0.0，仅 JS/TS/Python；无 CI 集成文档。

**集成方案**：
- `diffsense diff --summary` 的 JSON 输出可喂给 PR 审查流程
- 程序化 API (`analyzeDiff`, `diffFile`) 可嵌入现有工具链
- 与 ADV 的 hub_impact 分析互补：hub_impact 看调用图影响面，DiffSense 看变更语义风险

**前置条件**：需要在真实 PR 上验证风险评分的区分度。

### 8. Codexray — TS 健康检查

**核心价值**：10 条内置规则（god-module, no-large-functions, max-params 等），A-F 评分。

**限制**：仅 TypeScript；无 CI 文档。

**集成方案**：
- `codexray scan --format json` 输出可解析
- 规则与 ADV 的 `spec/DESIGN-REVIEW.md` 有重叠

**前置条件**：评估 ADV 的 TS 代码量是否值得引入专用工具。

---

## 第三梯队：暂不接入

### 9. Typewise

v0.1.1，仅 TS，文档极少。与 Codexray/Fallow 功能重叠，无差异化优势。

### 10. Reality

仅 React/Next.js，无公开 GitHub 仓库，文档稀疏。ADV 不是 React 项目。

### 11. pyvivo

研究原型，需 Python 3.14+ PEP 768。热补丁运行中进程的场景在 ADV 不成立（ADV 是 MCP 工具集，不是长驻服务）。

### 12. propre

v0.1.1，GitHub 仓库 404，文档几乎为零。"vibe code → production" 的定位模糊，无法评估可靠性。

---

## 推荐接入顺序

```
Phase 1（已完成 2026-10-02）:
  ✅ Vigilo — 已接入（scripts/vigilo_gate.py + spec/vigilo-baseline.json）
     基线棘轮：403 条存量入账，只拦新增。4 测试全绿，快门 3.7s。
  ❌ Skylos — 本机 MSVC 缺失，tree-sitter-dart-orchard 编不过。挂账。

Phase 2（需前置搭建）:
  ③ Fallow — 需 npm 生态，但 audit 模式与 god-ratchet 天然对齐
  ④ splice — 需先搭 Magellan 索引，但安全重构能力独一无二

Phase 3（观察等待）:
  ⑤ DiffSense — 等 v1.1+ 看 CI 集成
  ⑥ refute — 等 Rust 脱离 experimental
```

---

## 与现有 ADV 架构的接合点

| ADV 现有组件 | 可接入工具 | 接合方式 |
|-------------|-----------|---------|
| `local_gate.py` STEPS | Vigilo, Skylos | 新增门步骤，exit code 驱动 |
| `tools/lsp.py` | splice, refute | 作为 LS 下游消费者 |
| `god-baseline.json` | Fallow, Metrolith | 新度量轴（健康度/复杂度） |
| `hallucination_guard` | Skylos verify | 互补验证层 |
| `hub_impact.py` | DiffSense | 影响面 + 语义风险双维 |
| `bug_scan` | Vigilo | 功能正确性 + 安全漏洞正交 |
| `core.yml` CI | Fallow audit, Skylos cicd | 增量门 |

---

## 风险与注意事项

1. **工具成熟度**：12 个工具中 8 个版本 < 1.0，API 可能变。建议锁定版本、不追新。
2. **Python 版本**：Metrolith 需 3.13、pyvivo 需 3.14+，与 ADV 本机环境（3.14 裸装、3.11 有 numpy）冲突。
3. **额外守护进程**：splice 需 Magellan 索引守护进程，需评估资源开销与 LRU session cap 的交互。
4. **重叠风险**：Skylos/Fallow/Codexray 在死代码检测上有功能重叠，选定一个为主、其余为辅助验证。
5. **门链膨胀**：每加一个工具 = 加一条门 = 加一层可能 fail 的面。遵循 canary-gate playbook 纪律：先扩展现有门，不建新门。
