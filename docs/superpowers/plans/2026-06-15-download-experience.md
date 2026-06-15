# Download Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make collaborator downloads easier to set up, observe, and recover from in the Tauri desktop client.

**Architecture:** Keep the existing direct Rust/rclone download boundary. Add small backend commands for rclone checking and download-directory resolution, then move download output from a single log string to structured per-file results consumed by React.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, Vite, Vitest, rclone, Git.

---

### Task 1: Collaborator Quickstart

**Files:**
- Create: `../TYUT-ebooks-collection-neo/docs/collaborator-quickstart.md`
- Modify: `../TYUT-ebooks-collection-neo/README.md`
- Modify: `README.md`

- [ ] Write a short guide covering rclone install, read-only token setup, neo clone, GUI launch, update manifest, and download.
- [ ] Correct stale GUI repository names to `ebook-neo-desktop`.
- [ ] Run markdown/reference checks with `rg`.
- [ ] Commit docs in the affected repository.

### Task 2: Rclone Check And Open Folder

**Files:**
- Modify: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`

- [ ] Add failing Rust tests for `check_rclone_remote` and download-directory resolution.
- [ ] Implement backend commands.
- [ ] Add failing frontend tests for invoking check/open actions.
- [ ] Wire buttons into the right panel.
- [ ] Run frontend and Rust checks.
- [ ] Commit GUI changes.

### Task 3: Directory-Level Selection

**Files:**
- Modify: `src/manifest.ts`
- Modify: `src/manifest.test.ts`
- Modify: `src/App.tsx`

- [ ] Add failing tests for selecting all records under the active folder.
- [ ] Implement helper logic.
- [ ] Wire a table action that selects all matching folder records, not only the first 500 visible rows.
- [ ] Run frontend checks.
- [ ] Commit GUI changes.

### Task 4: Structured Download Results And Retry Failed

**Files:**
- Modify: `src-tauri/src/manifest.rs`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`
- Modify: `README.md`

- [ ] Add failing Rust tests for structured results and continuing after one file fails.
- [ ] Implement per-file `DownloadItemResult` output.
- [ ] Add failing frontend tests for rendering failed rows and retrying only failed paths.
- [ ] Wire retry action in the right panel.
- [ ] Update README download behavior.
- [ ] Run frontend/Rust checks and capture a screenshot.
- [ ] Commit GUI changes.

