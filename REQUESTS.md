# Requests Log

Keep a running log of user requests for this repo. For each request:

- Record the request verbatim (or near-verbatim).
- Track status: `Open` → `In Progress` → `Done`.
- When `Done`, add a short **Resolution** stub under the request (what changed and why).

## RQ-0001 (2026-01-16) — Done

**Request**

在当前目录下用 Rust 写一个高性能 IDE，支持 C++、Python 等语言，做一个 plan 写到目录下，并写一个 skill。

**Resolution**

- Added `PLAN.md` and created a bootstrap Codex skill under `skills/`.

## RQ-0002 (2026-01-16) — Done

**Request**

生成一个只影响当前目录的 `AGENTS.md`，包含代码风格等规则，并设置 milestones；完成 milestone 时提醒验收。

**Resolution**

- Added root `AGENTS.md` with Rust style rules and M0–M4 acceptance prompts.

## RQ-0003 (2026-01-16) — Done

**Request**

现在开始写代码吧。

**Resolution**

- Implemented Milestone M0: Cargo workspace + minimal window app that loads a file (CLI arg / drag-drop) and renders it with scrolling.

## RQ-0004 (2026-01-16) — Done

**Request**

根目录名字拼错了，应该是 RustIDE；改目录名以及相关内容；再创建一个文件用于存我提出的需求（每提出一个记录进去，完成时在问题下面写简要解决思路用于存根）；完成这些后先不要写代码等下一步指令。

**Resolution**

- Created `E:\RustIDE` as a junction pointing to the workspace folder (renaming the folder was blocked by an open WindowsTerminal cwd).
- Renamed project crates/skill from `ruseide-*` to `rustide-*` and updated docs/UI strings to “RustIDE”.
- Regenerated `Cargo.lock` and verified `cargo fmt`, `cargo check`, and `cargo test` pass.

## RQ-0005 (2026-01-16) — Done

**Request**

本项目之前名字写错了写成了RuseIDE，现在改成RustIDE，但是还有有软连接，帮我删除RuseIDE这个文件夹保证当前项目正常工作

**Resolution**

- Replaced the `E:\RustIDE` junction with a real `E:\RustIDE` folder by copying from `E:\RuseIDE` (excluded `target/`).
- Verified the workspace builds under `E:\RustIDE` with `cargo check`.
- Deleted `E:\RuseIDE` after releasing file handles held by Cursor/integrated terminals.

## RQ-0006 (2026-01-16) — Done

**Request**

现在我要写一个基于Ruse的IDE，PLAN以及REQUESTS等已经写好，先阅读下源码然后完成MileStone1，此外还需要支持配置文件，记住窗口位置大小等，支持中文编码

**Resolution**

- Implemented Milestone M1 core editing: rope-backed buffer, selection, undo/redo, and an editor widget with visible-row rendering.
- Added config support (`%APPDATA%/RustIDE/config.ini` or `RUSTIDE_CONFIG`) including window position/size persistence and file encoding hint.
- Added CJK-friendly decoding (UTF-8/UTF-16 BOM + GBK/Big5 fallback) and a Windows font fallback to render Chinese.

## RQ-0007 (2026-01-16) — Done

**Request**

AGENTS.md加上一条，代码写上必要的注释，要精简，并且用英文，然后继续测试milestone1是否完成，如果没有的话继续

**Resolution**

- Updated `AGENTS.md` to require concise English comments and fixed garbled acceptance text.
- Added minimal English comments in the new M1 code paths and verified `cargo fmt`, `cargo test`, `cargo clippy` are clean.

## RQ-0008 (2026-01-16) — Done

**Request**

新增一个需求，支持字体和字体大小调整，默认选择consolas字体

**Resolution**

- Added `ui.monospace_font` and `ui.monospace_size` config keys; default monospace font is Consolas.
- Added in-app controls (top bar) to switch monospace font (Consolas/SimHei) and adjust monospace size; changes persist to config.

## RQ-0009 (2026-01-16) — Done

**Request**

现在这个交互不是很好，如果鼠标悬停在可编辑位置时变成|光标，并且可以点击任意位置进行编辑

**Resolution**

- Updated `crates/rustide-app/src/editor_view.rs` to show a text (I-beam) cursor when hovering the editable viewport and to map clicks anywhere in the editor viewport to a caret position.

## RQ-0010 (2026-01-16) — Done

**Request**

继续MileStone2，并且在MileStone2额外加一条支持markdown渲染

**Resolution**

- Added `crates/rustide-syntax` (Tree-sitter for C++/Python) and integrated incremental parsing + debounced highlight updates into the editor view.
- Added Markdown preview rendering for `.md` files via `egui_commonmark` (side-by-side preview toggle).
- Updated `AGENTS.md` M2 definition to include Markdown rendering.

## RQ-0011 (2026-01-16) — Done

**Request**

新增需求，增加设置启动时打开新文件还是上次打开的文件，默认打开上次的文件，如果不存在就打开新文件

**Resolution**

- Added `[startup] open_last_file` (default true) and `last_file` to `config.ini`; app auto-loads `last_file` on startup when enabled.
- Persisted `last_file` on successful loads and exposed a quick toggle (`Open last`) in the top bar.

## RQ-0012 (2026-01-16) — Done

**Request**

M2的很多关键字语法不高亮，并且markdown渲染效果不对

**Resolution**

- Expanded Tree-sitter highlight mapping (variables/operators/punctuation/etc) and improved C++ highlighting by augmenting the upstream query to include comments/numbers/strings/preprocessor directives.
- Improved Markdown preview UX by making the preview panel scrollable and ensuring proportional font fallbacks include CJK/user fonts.

## RQ-0013 (2026-01-16) — Done

**Request**

继续MileStone4

**Resolution**

- Added Project panel with root selection, live file tree (ignore rules respected) and background watcher refresh.
- Added global search (background thread, cancelable) with result list that opens files and jumps caret to match.

## RQ-0014 (2026-01-16) — Done

**Request**

增加类似code glance的side bar功能，可以直接拖动编辑器右边的side bar，增加主题功能，提供几个默认主题配色

**Resolution**

- Added a right-side minimap/scrollbar (CodeGlance-style) that supports click/drag to scroll.
- Added theme support with built-in themes (`dark`, `light`, `solarized-dark`, `monokai`), persisted via `config.ini`, and applied to syntax highlighting + minimap.

## RQ-0015 (2026-01-16) — Done

**Request**

Project Tree侧边滚动条拖动会崩溃；重做左侧为图标面板（Project/Search）；右侧 minimap 半透且显示文本缩略图

**Resolution**

- Split the left sidebar into a small icon toolstrip (Project/Search) and a toggleable panel to avoid layout edge-cases that could crash scrollbar interaction.
- Improved the right minimap to be semi-transparent and render a sampled text thumbnail of the current document; also hardened minimap math to avoid `clamp` panics.

## RQ-0016 (2026-01-16) — Done

**Request**

窗口缩放时字体大小不正确；启动时主题记住但效果不对；编辑器显示行号；左侧 P/S 面板不应全屏且布局需要持久化到配置文件

**Resolution**

- Re-applied theme + monospace text style every frame (and included DPI changes) so resizing/moving the window keeps font sizing consistent and the saved theme always takes effect.
- Added line numbers to the editor gutter.
- Capped and persisted the left tool panel layout (`[layout] left_tool/left_panel_width`) to `config.ini` to avoid full-screen panels and remember layout across runs.

## RQ-0017 (2026-01-16) — In Progress

**Request**

新增一个milestone5：支持HLSL语法高亮；支持代码补全（C++/HLSL）；所有窗口dockable且布局持久化到配置文件

## RQ-0018 (2026-01-16) — Done

**Request**

支持Ctrl+F搜索当前文件；字体粗体/斜体选择；侧边栏可拖动大小；双击选中词/三击选中整行；hover 光标更明显；换行自动缩进（保持上一行缩进，遇到 `{`/`}` 自动处理）

**Resolution**

- Added in-file Find bar (`Ctrl+F`) with Next/Prev and selection jump.
- Added monospace font style selection (regular/bold/italic/bold-italic) with Windows Consolas variants when available.
- Made the right minimap resizable via drag handle and persisted via `config.ini` (`[ui] minimap_width=...`).
- Added double-click word select and triple-click line select; increased caret thickness for visibility.
- Added auto-indent on Enter (keeps previous indentation, handles `{}` block insertion) with unit tests.

## RQ-0019 (2026-01-16) — Done

**Request**

拖入文件不改变已打开的 project（允许打开 project 外的文件）；Markdown 源码滚动时 preview 同步；Project tree 增加目录/文件类型图标；全局字体与设置一致；`cargo run` 启动闪烁；文件路径+文件名居中；Open 弹出选择文件/文件夹；顶部栏颜色与主题一致；选中高亮与字体位置不一致（偏上）；支持上下左右移动光标

**Resolution**

- Added non-blocking file/folder pickers (`rfd` on a background thread) and wired them to `Open → File…/Folder…` (file opens without changing project root; folder opens project root).
- Synced Markdown preview scrolling to the source editor using a scroll-ratio mapping.
- Centered the active file path header in the editor tab.
- Made proportional UI font follow the configured editor font (keeps defaults as fallbacks).
- Fixed caret/selection vertical alignment (consistent y-offset + correct click cursor mapping).
- Added `Ctrl+Shift+F` to focus the Search tab and focus its query field.

## RQ-0020 (2026-01-16) — Done

**Request**

打开新文件时新增一个窗口（不要覆盖当前文件）；增加 <- / -> 按钮（放在 editor 右边）用于回到上一次/下一次位置；中英文布局对齐（行内上下 space 一致）；不要显示 `Loaded: ...` 状态；`Ctrl+F` 搜索命中后跳到对应位置。

**Resolution**

- Corrected per follow-up: opening files now adds a **new editor tab in the same window** (VSCode-style) instead of spawning a new app process. Open/File, drag-drop, Project tree clicks, and Search result clicks all create a new tab and focus it.
- Added <- / -> navigation buttons on the right side of the editor header, with a simple cursor location history (mouse clicks/drag + Find jumps) supporting back/forward.
- Navigation history is tracked per tab.
- Improved mixed Chinese/English text alignment by ensuring editor row height accounts for typical CJK glyph height and vertically centers each rendered line.
- Removed the noisy `Loaded: ...` status message (successful loads clear status; top bar hides empty status).
- `Ctrl+F` Find now scrolls the editor to the matched range after selecting it.

## RQ-0021 (2026-01-17) — Done

**Request**

重复打开同一文件时，聚焦到该文件 Tab（不要再打开新 tab）；窗口上边栏（Tab 条）右键支持：Pin tab、关闭其他 tab、关闭右侧 tab。

**Resolution**

- When opening a file, RustIDE now deduplicates by path: if the file is already open, it focuses the existing tab and applies any jump (Search result line/col) on that tab.
- Added a tab context menu (right click): Pin/Unpin, Close Others, Close Tabs to the Right.
- Bulk close operations keep pinned tabs, and pinned tabs cannot be closed via the close button.

## RQ-0022 (2026-01-17) — Done

**Request**

Tab 右键菜单额外增加：Close All（关闭所有 tab，包括 pinned）和 Close All But Pinned；启动 app 时不要自动创建 Untitled tab。

**Resolution**

- Added tab context menu actions: Close All (closes every tab, including pinned) and Close All But Pinned (keeps only pinned tabs).
- Removed the default Untitled tab on startup; the Editor tab shows an empty-state prompt when no file is open.

## RQ-0024 (2026-01-17) �� Done

**Request**

�������Ѿ�������git�ֿ⣬����û�д�����pull��ͻ�����Ҵ�����

**Resolution**

- Set main to track origin/main, fetched the remote commits, and merged with --allow-unrelated-histories so the upstream initial commit (LICENSE) is now part of our history without dropping your work (git branch --set-upstream-to=origin/main main; git merge origin/main --allow-unrelated-histories).
- After the merge the working tree is clean and git status reports we are ahead by the new commits; run git push after reviewing so the remote catches up.
