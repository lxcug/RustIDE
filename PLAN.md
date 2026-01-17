# RustIDE：Rust 高性能 IDE 计划（草案）

## 1. 目标与边界

- **目标**：在 Windows 优先（可扩展到 macOS/Linux）的前提下，用 Rust 构建一个高性能 IDE，首批支持 **C++**、**Python**（后续可扩展到 Rust/Go/TS 等）。
- **核心原则**：
  - **编辑器体验优先**：大文件（>50MB 级别）仍保持流畅滚动与输入响应。
  - **语言能力解耦**：语言特性以 **LSP** 为主，IDE 只实现通用管线。
  - **异步与隔离**：索引、诊断、git、搜索等重任务不阻塞 UI；语言服务尽量进程隔离。
- **不做/暂缓**（MVP 外）：内置编译器/解释器、复杂 refactor 引擎（由 LSP 提供）、跨端同步等。

## 2. MVP（最小可用版本）定义

MVP 的“能用”标准（可验证）：

- 打开/编辑/保存文件；多标签页；最近文件。
- 基础编辑能力：撤销/重做、复制粘贴、选择、查找/替换、行号、缩进。
- 语法高亮（Tree-sitter）+ 基础括号/引号匹配。
- LSP：启动/管理语言服务器；显示 diagnostics；hover；补全（至少 C++/Python 各一种常见服务器）。
  - C++：优先 `clangd`（基于 `compile_commands.json`）。
  - Python：优先 `pyright`（Node 生态），备选 `pylsp`。
- 项目视图：打开文件夹；文件树；文件监听刷新。
- 全局搜索：ripgrep 风格的文本搜索（可在后台执行）。

## 3. 技术选型（初稿，可迭代）

- **UI/渲染**：`winit` + `wgpu`（GPU 渲染）；UI 组件可先用 `egui` 快速成型，但编辑器文本视图建议做 **自定义高性能 widget**。
- **文本模型**：`ropey`（Rope）+ 自研行/列索引缓存；撤销/重做使用操作日志（op-log）。
- **文本排版**：优先 `cosmic-text`（形状、合字、IME 友好）；必要时做 glyph cache。
- **语法**：`tree-sitter`（增量解析），用于高亮、折叠、结构选择等。
- **LSP/JSON-RPC**：`tokio` + `serde_json` + `lsp-types`，实现轻量客户端与进程管理（stdio）。
- **文件系统与忽略**：`notify` + `ignore`（.gitignore 语义）。
- **配置**：`toml`（`serde`），键位与命令用声明式映射（可热加载）。

## 4. 架构拆分（建议 Cargo workspace）

建议从一开始就以 workspace 组织，避免单仓单 crate 变胖：

- `crates/rustide-app`：入口、生命周期、窗口/事件循环。
- `crates/rustide-ui`：UI 框架适配、布局、主题、命令面板。
- `crates/rustide-editor`：buffer/selection/undo、渲染层接口、编辑命令。
- `crates/rustide-syntax`：tree-sitter 集成、高亮与折叠信息。
- `crates/rustide-lsp`：LSP client、server manager、能力缓存（completion/hover/diagnostics）。
- `crates/rustide-project`：workspace、项目根、语言配置、构建信息（CMake/compile_commands）。
- `crates/rustide-vfs`：虚拟文件系统、文件 watcher、去抖与事件合并。
- `crates/rustide-plugin-api`：插件 API（后续可选 WASM/动态库）。

关键数据流（高层）：

- UI 线程：输入事件 → 命令 → 编辑器状态变更 → 渲染（尽量不阻塞）
- 后台任务：索引/搜索/LSP I/O → 事件队列 → 主线程合并状态（batch 更新）

## 5. 里程碑（建议）

- **M0：脚手架**：workspace + 基础窗口 + 日志 + 配置加载；能打开单文件并显示内容。
- **M1：编辑器内核**：Rope buffer、光标/选择、undo/redo、基础渲染与滚动。
- **M2：语法高亮**：Tree-sitter 增量解析 + 高亮 pipeline + 主题。
- **M3：LSP 集成**：clangd/pyright 启动与管理；diagnostics、hover、completion；请求去抖与取消。
- **M4：项目与搜索**：文件树、watcher、全局搜索、最近项目。
- **M5：体验打磨**：IME、字体渲染缓存、性能剖析与优化、崩溃恢复（自动保存）。
- **M6：扩展与调试（可选）**：DAP、插件系统、任务系统（build/run/test）。

## 6. 性能与工程化要点（早期就要做）

- **渲染只画可见区域**：视口裁剪 + 行缓存 + glyph cache。
- **增量更新**：编辑引起的解析/高亮/LSP 更新都要有去抖与取消。
- **事件批处理**：watcher/LSP 的 burst 更新合并成小批次提交 UI。
- **基准与剖析**：从 M1 开始建立 micro-bench（大文件滚动/输入延迟）。

## 7. 下一步

- 使用本仓库内的 Codex skill：`skills/rustide-bootstrap/SKILL.md`，按其中的流程初始化 workspace，并实现 M0/M1。
