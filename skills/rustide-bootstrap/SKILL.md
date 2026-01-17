---
name: rustide-bootstrap
description: Bootstrap and iterate on the Rust-based high-performance IDE project in this repo (RustIDE). Use when Codex needs to (1) scaffold the Cargo workspace and crate boundaries, (2) choose/lock the UI+rendering and text stack, (3) implement the MVP editor loop, buffer/undo, and rendering, or (4) integrate multi-language features via LSP (especially C++ via clangd and Python via pyright/pylsp).
---

# RustIDE Bootstrap

## Goal

Create an extensible Rust IDE foundation that stays responsive under large files and gets language features through LSP, aligned with `PLAN.md`.

## Defaults (change only if requested)

- UI/rendering: `winit` + `wgpu`; use `egui` for shell panels early; implement the editor view as a custom high-performance widget.
- Async: `tokio` runtime; background tasks communicate with UI via channels; batch UI updates.
- Text model: `ropey` (Rope) + cached line/byte mapping; op-log undo/redo.
- Shaping: prefer `cosmic-text` for correct shaping/IME behavior.
- Syntax: `tree-sitter` incremental parsing for highlighting/folding.
- Language: external language servers over stdio (JSON-RPC / LSP).

## Workflow

### 1) Confirm prerequisites

- Optionally run `skills/rustide-bootstrap/scripts/check_prereqs.ps1`.
- Confirm: Rust toolchain, Git, and at least one LSP server:
  - C++: `clangd`
  - Python: `pyright-langserver` (Node) or `pylsp` (Python)

### 2) Align with the repo plan

- Read `PLAN.md` and pick the milestone scope to implement next (prefer M0 → M3 order).
- Write down explicit acceptance criteria before coding (or use `skills/rustide-bootstrap/references/mvp.md`).

### 3) Scaffold the workspace (M0)

- Create a Cargo workspace with crates aligned to `PLAN.md` (app/ui/editor/syntax/lsp/project/vfs/plugin-api).
- Ensure a minimal app boots: create window, event loop, open a file, and render text (even if initially unstyled).

### 4) Build the editor core (M1)

- Implement buffer/selection/undo as a library crate with unit tests.
- Render only the visible viewport; cache line breaks and glyphs; avoid per-frame allocations.

### 5) Add syntax pipeline (M2)

- Integrate tree-sitter with incremental updates on edits.
- Map parse highlights → theme tokens → draw calls.

### 6) Add LSP manager (M3)

- Implement a process manager: start/stop/restart servers per workspace + language.
- Add request scheduling: debounce typing-triggered requests, support cancellation, cap concurrency.
- Start with: diagnostics + hover + completion.
- Provide adapters/config defaults:
  - C++: detect `compile_commands.json` and pass workspace root to `clangd`.
  - Python: prefer `pyright-langserver --stdio`; fall back to `pylsp` if configured.

### 7) Keep performance a first-class requirement

- Never block the UI thread on I/O.
- Batch/merge frequent events (watcher + LSP diagnostics bursts).
- Add micro-benchmarks early for buffer edits and viewport rendering hot paths.

## Reference material

- `PLAN.md`: overall milestones and crate split.
- `skills/rustide-bootstrap/references/mvp.md`: acceptance criteria checklist for M0–M3 and performance guardrails.
