# Scoped Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let collaborators scan/sync only the currently selected folder and open the configured sync folder from the GUI.

**Architecture:** Extend the existing sync request contract with an optional manifest folder prefix. Keep all path validation and directory opening in Rust, and keep React responsible for selecting the active scope and passing it to backend commands.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, Vitest, Testing Library.

---

### Task 1: Backend Scoped Sync

**Files:**
- Modify: `src-tauri/src/manifest.rs`

- [x] Add failing Rust tests:
  - scoped sync only counts/downloads records below `资料/数据结构`
  - invalid scope prefix such as `../escape` is rejected
- [x] Run targeted tests and confirm they fail.
- [x] Add `scope_prefix: Option<String>` to `SyncPlanRequest`.
- [x] Filter manifest records with exact folder-boundary matching before `build_sync_plan_for_records`.
- [x] Run targeted Rust tests and full `cargo test`.
- [x] Commit backend slice.

### Task 2: Frontend Scoped Sync And Open Sync Folder

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`

- [x] Add failing frontend tests:
  - choosing current-folder scope sends `scopePrefix` equal to the active folder
  - start sync downloads only scoped `downloadPaths` into `syncRoot`
  - open sync folder calls `open_download_root` with `downloadRoot: syncRoot`
- [x] Run targeted frontend tests and confirm they fail.
- [x] Add sync scope UI and derive `scopePrefix` from `activeFolder`.
- [x] Disable scoped scan/start when current-folder mode has no active folder.
- [x] Add `打开同步目录` button and reuse the status/log feedback path.
- [x] Run targeted frontend tests and full `npm test`.
- [x] Commit frontend slice.

### Task 3: Verification And Docs

**Files:**
- Modify: `README.md`
- Add: `.agent/visual/ebook-neo-scoped-sync.png`
- Add: `.agent/visual/ebook-neo-scoped-sync.md`
- Modify: workspace `memory/*.md`

- [x] Document scoped sync and opening the sync folder.
- [x] Run `npm test`.
- [x] Run `npm run build`.
- [x] Run `npm audit --audit-level=moderate --registry=https://registry.npmjs.org`.
- [x] Run `cargo fmt --check` in `src-tauri`.
- [x] Run `$env:CARGO_BUILD_JOBS='1'; cargo test` in `src-tauri`.
- [x] Run `$env:CARGO_BUILD_JOBS='1'; cargo check` in `src-tauri`.
- [x] Run `git diff --check`.
- [x] Capture visual evidence for the scoped sync view.
- [x] Commit docs/evidence and update memory.
