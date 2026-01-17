# RustIDE MVP checklist (M0–M3)

Use this as “done means done” criteria when implementing the first milestones from `PLAN.md`.

## M0 — Boot & shell

- App boots to a window, logs to console/file, and can open a file path (CLI arg or simple dialog later).
- Renders the file contents (monospace font), with scroll support.
- Separation exists between UI shell and editor core crates (even if minimal).

## M1 — Editor core

- Buffer supports: insert/delete, multi-line, undo/redo, selection, cursor movement (basic arrows + word jump optional).
- Viewport rendering draws only visible lines; line measurement cached; no O(n) per-frame scanning of entire file.
- Editing large files stays responsive (define a target, e.g. keypress-to-paint under ~16ms on a reasonable machine).
- Unit tests cover buffer invariants and undo/redo correctness.

## M2 — Syntax highlighting

- Tree-sitter incremental parse updates based on edits (not full reparse every keystroke).
- Highlight pipeline:
  - language detection (by extension / shebang)
  - parse tree → highlight spans
  - spans → theme tokens → rendering
- Highlight updates are debounced and cancellable; UI never blocks on parsing.

## M3 — LSP integration

### Process management

- Start language servers per workspace root and language.
- Restart on crash with backoff; surface errors non-fatally in UI.
- Wire stdio JSON-RPC with request IDs; handle notifications.

### Features (minimum)

- Diagnostics: publish and render in gutter/underline; clickable message list.
- Hover: show a popup near cursor.
- Completion: invoke manually and (optionally) after typing with debounce; include snippet/textEdit support when present.

### C++ (clangd)

- Detect `compile_commands.json` in workspace; use it if present.
- Respect multi-root projects (later), but at least single-root works reliably.

### Python (pyright / pylsp)

- Prefer `pyright-langserver --stdio` if available; otherwise support `pylsp`.
- Use workspace root for config discovery; allow overriding server command via config file.

## Performance guardrails (apply from day 1)

- UI thread never does blocking I/O; use async tasks + channels.
- Batch frequent external events (file watcher, diagnostics storms).
- Add profiling hooks (feature flag) and at least one micro-bench for:
  - buffer insert/delete in the middle of large file
  - viewport render cost for long lines and many visible lines
