# Agent Instructions (RustIDE root)

These instructions are for Codex agents working in `e:\RustIDE` (junction to the workspace).

## Working Agreement

- Keep changes **milestone-scoped**; avoid unrelated refactors.
- Prefer **simple, measurable** implementations over speculative abstractions.
- When a milestone is finished, pause and ask the user for acceptance using the checklist and commands below.
- Track every user request in `REQUESTS.md` (append new request; when done, add a short Resolution stub under it).
- Record each request and the planned solution in `REQUESTS.md` before coding; update the same entry with a Resolution stub when the work is done.
- Windows-first, but avoid hard-coding Windows paths unless required.

## Code Style (Rust)

- Edition: Rust **2021** (unless the repo already uses a different edition).
- Formatting: run `cargo fmt` (no manual formatting wars).
- Linting: keep `cargo clippy --all-targets --all-features` clean; prefer fixing root causes.
- Error handling: use `thiserror` for typed errors in libraries and `anyhow` for app-level wiring.
- Logging: use `tracing` (`tracing_subscriber`) rather than `println!`.
- Concurrency: use `tokio` and channels; **never block the UI thread** on I/O.
- Naming: avoid 1-letter names; prefer explicit types and clear module boundaries.
- Tests: add unit tests for pure logic (buffer/undo/rope mapping). Avoid UI snapshot tests early.
- Comments: add minimal necessary comments, concise, and in English.

## Repo Structure Expectations

- Use a Cargo workspace and small crates (app/ui/editor/syntax/lsp/project/vfs).
- Keep UI-facing state updates **batched** (coalesce file watcher + LSP bursts).
- Rendering: draw only the visible viewport; avoid per-frame allocations and full-file scans.

## Milestones & Acceptance

### M0 — Boot & File Open

**Done when:**
- App boots to a window and can open a file path (CLI arg is OK).
- File contents render and scrolling works.

**Acceptance (user runs):**
- `cargo run -- <path-to-some-text-file>`

When M0 completes: ask the user to confirm boot + open + scroll.

### M1 — Editor Core (Buffer/Selection/Undo)

**Done when:**
- Insert/delete, multi-line edits, selection, undo/redo work reliably.
- Viewport rendering stays responsive on large files; UI thread does not block on I/O.
- Unit tests cover buffer invariants and undo/redo.

**Acceptance (user runs):**
- `cargo test`
- Manual: open a large file, type, undo/redo, scroll.

When M1 completes: ask the user to confirm editing/undo correctness and that it feels fast.

### M2 — Syntax Highlight (Tree-sitter)

**Done when:**
- Incremental parse/highlight updates on edits (no full reparse per keystroke).
- Theme mapping works; highlight updates are debounced/cancellable.
- Markdown rendering is supported (preview mode is OK).

**Acceptance (user runs):**
- Manual: open `.cpp` and `.py`, verify highlight updates while typing.

When M2 completes: ask the user to confirm correctness + no noticeable input lag.

### M3 — LSP (C++ clangd, Python pyright/pylsp)

**Done when:**
- LSP processes start/stop reliably (stdio JSON-RPC).
- Diagnostics + hover + completion work for C++ and Python.
- clangd uses `compile_commands.json` when present; Python server command is configurable.

**Acceptance (user runs):**
- Install servers if needed:
  - `clangd` (LLVM/clang tools)
  - `npm i -g pyright`
- Manual: open a small C++ project + a Python folder; verify diagnostics/hover/completion.

When M3 completes: ask the user to confirm both languages work end-to-end.

### M4 — Project Tree & Search

**Done when:**
- Open folder shows file tree with live updates (watcher + ignore rules).
- Global search works (background task, cancelable).

**Acceptance (user runs):**
- Manual: rename/add files, confirm tree updates; run a search and open results.

When M4 completes: ask the user to confirm watcher stability and search performance.

### M5 — Docking + HLSL + Completion

**Done when:**
- HLSL files (`.hlsl`, `.hlsli`, `.fx`) have Tree-sitter syntax highlighting (incremental, debounced like M2).
- Code completion works for C++ and HLSL (LSP-based), with a manual trigger (e.g. `Ctrl+Space`) and while typing.
- All major panes are dockable (drag tabs, split, resize), and the full layout persists to `config.ini`.

**Acceptance (user runs):**
- Install servers if needed:
  - `clangd` (LLVM/clang tools)
  - An HLSL LSP server (configurable), e.g. `hlsl-language-server`
- Manual:
  - Open a `.cpp` and a `.hlsl`, verify highlighting.
  - Trigger completion in both languages and verify the popup inserts text.
  - Drag panes into a custom layout, restart the app, confirm the layout is restored.

When M5 completes: ask the user to confirm HLSL highlight, completion UX, and layout persistence.
