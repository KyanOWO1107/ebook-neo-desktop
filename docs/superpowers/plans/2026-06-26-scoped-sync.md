# Scoped Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let collaborators scan/sync only the currently selected folder and open the configured sync folder from the GUI.

**Architecture:** Extend the existing sync request contract with an optional manifest folder prefix. Keep all path validation and directory opening in Rust, and keep React responsible for selecting the active scope and passing it to backend commands.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, Vitest, Testing Library.

---

### Task 1: Backend Scoped Sync

**Files:**
- Modify: `src-tauri/src/manifest.rs`

- [ ] Add failing Rust tests:
  - scoped sync only counts/downloads records below `资料/数据结构`
  - invalid scope prefix such as `../escape` is rejected
- [ ] Run targeted tests and confirm they fail.
- [ ] Add `scope_prefix: Option<String>` to `SyncPlanRequest`.
- [ ] Filter manifest records with exact folder-boundary matching before `build_sync_plan_for_records`.
- [ ] Run targeted Rust tests and full `cargo test`.
- [ ] Commit backend slice.

### Task 2: Frontend Scoped Sync And Open Sync Folder

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`

- [ ] Add failing frontend tests:
  - choosing current-folder scope sends `scopePrefix` equal to the active folder
  - start sync downloads only scoped `downloadPaths` into `syncRoot`
  - open sync folder calls `open_download_root` with `downloadRoot: syncRoot`
- [ ] Run targeted frontend tests and confirm they fail.
- [ ] Add sync scope UI and derive `scopePrefix` from `activeFolder`.
- [ ] Disable scoped scan/start when current-folder mode has no active folder.
- [ ] Add `打开同步目录` button and reuse the status/log feedback path.
- [ ] Run targeted frontend tests and full `npm test`.
- [ ] Commit frontend slice.

### Task 3: Verification And Docs

**Files:**
- Modify: `README.md`
- Add: `.agent/visual/ebook-neo-scoped-sync.png`
- Add: `.agent/visual/ebook-neo-scoped-sync.md`
- Modify: workspace `memory/*.md`

- [ ] Document scoped sync and opening the sync folder.
- [ ] Run `npm test`.
- [ ] Run `npm run build`.
- [ ] Run `npm audit --audit-level=moderate --registry=https://registry.npmjs.org`.
- [ ] Run `cargo fmt --check` in `src-tauri`.
- [ ] Run `$env:CARGO_BUILD_JOBS='1'; cargo test` in `src-tauri`.
- [ ] Run `$env:CARGO_BUILD_JOBS='1'; cargo check` in `src-tauri`.
- [ ] Run `git diff --check`.
- [ ] Capture visual evidence for the scoped sync view.
- [ ] Commit docs/evidence and update memory.
