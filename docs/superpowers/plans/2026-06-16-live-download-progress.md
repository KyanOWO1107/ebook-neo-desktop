# Live Download Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add live download progress and cancellation to the Tauri desktop client without changing the R2 or manifest model.

**Architecture:** Keep direct Rust `rclone cat` downloads and the existing safe temp-file install path. Add a task manager around the existing downloader, emit `download-progress` events from the backend, and update React state from those events.

**Tech Stack:** Tauri v2, Rust stable, React 19, TypeScript, Vite, Vitest, rclone.

---

### Task 1: Backend Download Task Model

**Files:**
- Modify: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add failing Rust tests**

Add tests proving:

```rust
#[test]
fn progress_event_payload_tracks_file_and_batch_counts() {
    let event = download_progress_event(
        "task-1",
        "progress",
        Some("资料/a.txt"),
        2,
        3,
        0,
        0,
        1,
        "streaming 资料/a.txt",
    );

    assert_eq!(event.task_id, "task-1");
    assert_eq!(event.kind, "progress");
    assert_eq!(event.path.as_deref(), Some("资料/a.txt"));
    assert_eq!(event.bytes_written, 2);
    assert_eq!(event.total_bytes, 3);
    assert_eq!(event.completed_files, 0);
    assert_eq!(event.failed_files, 0);
    assert_eq!(event.total_files, 1);
}
```

Run:

```powershell
cargo test progress_event_payload_tracks_file_and_batch_counts
```

Expected: fail because `download_progress_event` does not exist yet.

- [ ] **Step 2: Implement event payload structs and helpers**

Add `DownloadTask`, `DownloadProgressEvent`, `DownloadTaskRegistry`, and helper functions for task ids, event payloads, and cancel flag lookup.

- [ ] **Step 3: Run Rust tests**

Run:

```powershell
cargo test progress_event_payload_tracks_file_and_batch_counts
```

Expected: pass.

- [ ] **Step 4: Add start/cancel command tests**

Add tests proving `start_download` and `cancel_download` are async command futures and that repeated cancel of an unknown task returns a clear error.

- [ ] **Step 5: Wire commands**

Register `start_download` and `cancel_download` in `src-tauri/src/lib.rs`.

- [ ] **Step 6: Commit backend task model**

Run:

```powershell
cargo fmt
cargo test download_task
git add src-tauri/src/manifest.rs src-tauri/src/lib.rs docs/superpowers/specs/2026-06-16-live-download-progress-design.md docs/superpowers/plans/2026-06-16-live-download-progress.md
git commit -m "feat: add download task backend"
```

### Task 2: Backend Streaming Progress And Cancellation

**Files:**
- Modify: `src-tauri/src/manifest.rs`

- [ ] **Step 1: Add failing streaming progress test**

Add a test with fake rclone output `abc` proving the downloader emits at least `started`, `progress`, and `finished` item events, and that the final file verifies.

- [ ] **Step 2: Implement progress-emitting copy loop**

Replace the one-shot `std::io::copy` path inside the new task downloader with a chunked loop that writes stdout, flushes the temp file, checks cancellation, and emits byte progress.

- [ ] **Step 3: Add failing cancellation test**

Add a test using a long-running fake rclone script proving cancellation returns a `canceled` item and removes `.ebook-neo-part`.

- [ ] **Step 4: Implement cancellation**

Store one cancel flag per task, check it before starting queued work and inside the stream loop, kill the active child on cancellation, and remove the temp file.

- [ ] **Step 5: Verify and commit**

Run:

```powershell
cargo fmt
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
git add src-tauri/src/manifest.rs
git commit -m "feat: stream download progress"
```

### Task 3: Frontend Live Progress UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`
- Modify: `README.md`

- [ ] **Step 1: Mock and test event subscription**

Mock `@tauri-apps/api/event.listen` and add a failing test proving the app listens to `download-progress`.

- [ ] **Step 2: Start downloads through the task command**

Change `downloadPaths` to call `start_download`, store `taskId`, and wait for events rather than waiting for a full `DownloadResult`.

- [ ] **Step 3: Render live progress**

Add compact progress UI in the download panel: overall bar, current file, byte count, completed/failed/canceled counts, and current task message.

- [ ] **Step 4: Add cancel action**

Add a cancel button while a task is active. It calls `cancel_download` with the current task id.

- [ ] **Step 5: Update README**

Document that downloads now show live progress and can be canceled; retry still applies to failed files only.

- [ ] **Step 6: Verify and commit**

Run:

```powershell
npm test
npm run build
git add src/App.tsx src/App.test.tsx src/App.css README.md
git commit -m "feat: show live download progress"
```

### Task 4: Final Verification And Visual Evidence

**Files:**
- Modify: `.agent/visual/*`
- Modify: `docs/superpowers/plans/2026-06-16-live-download-progress.md`

- [ ] **Step 1: Run full checks**

Run:

```powershell
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
Set-Location src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
Set-Location ..
git diff --check
```

- [ ] **Step 2: Capture screenshot evidence**

Run the Vite UI with a Tauri invoke/listen mock that displays live progress, capture a screenshot, and write a markdown note under `.agent/visual/`.

- [ ] **Step 3: Update plan checkboxes**

Mark completed steps in this plan.

- [ ] **Step 4: Commit final evidence**

Run:

```powershell
git add .agent/visual docs/superpowers/plans/2026-06-16-live-download-progress.md
git commit -m "test: verify live download progress"
```
