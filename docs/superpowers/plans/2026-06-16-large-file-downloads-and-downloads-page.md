# Large File Downloads And Downloads Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve large single-file download throughput with a configurable `rclone copyto` path and make multi-file download status stable through a dedicated downloads view.

**Architecture:** Backend transfer selection remains inside `src-tauri/src/manifest.rs`, choosing `cat` for small files and `copyto` for files at or above the configured threshold. Frontend settings live in `src/manifest.ts` and `src/App.tsx`; download queue state is owned by `App.tsx` and rendered differently in resources and downloads views.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, Vitest, Testing Library, rclone.

---

### Task 1: Settings And Backend Copyto Fast Path

**Files:**
- Modify: `src-tauri/src/manifest.rs`
- Modify: `src/manifest.ts`
- Modify: `src/manifest.test.ts`
- Modify: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `README.md`

- [ ] **Step 1: Write failing frontend settings tests**

Add assertions to `src/manifest.test.ts`:

```ts
expect(defaultAppSettings.largeFileThresholdMiB).toBe(20);
expect(defaultAppSettings.largeFileStreams).toBe(8);
expect(mergeAppSettings({ largeFileThresholdMiB: 0, largeFileStreams: 99 })).toMatchObject({
  largeFileThresholdMiB: 1,
  largeFileStreams: 16,
});
```

Add an `App.test.tsx` assertion that changing labels `大文件阈值` and `大文件线程` keeps inputs editable and sends those fields in the `start_download` request.

- [ ] **Step 2: Run tests to verify frontend red**

Run:

```powershell
npm test -- src/manifest.test.ts src/App.test.tsx
```

Expected: fails because the new settings fields and labels do not exist.

- [ ] **Step 3: Write failing Rust tests**

In `src-tauri/src/manifest.rs`, add tests that require:

```rust
assert_eq!(default_settings().large_file_threshold_mib, 20);
assert_eq!(default_settings().large_file_streams, 8);
```

Add one test for `build_rclone_copyto_args(...)` expecting:

```text
copyto <remote>:<bucket>/<object_key> <temp_path>
--multi-thread-streams 8
--multi-thread-cutoff 1M
--multi-thread-chunk-size 16M
--stats 1s
```

Add one fake-rclone test where a record above the threshold writes bytes to the destination argument passed to `copyto`, then verify the final file exists and `.ebook-neo-part` is gone.

- [ ] **Step 4: Run tests to verify Rust red**

Run:

```powershell
cd src-tauri
$env:CARGO_BUILD_JOBS='1'; cargo test large_file
```

Expected: fails because fields/helpers/copyto behavior do not exist.

- [ ] **Step 5: Implement settings defaults and validation**

In `src/manifest.ts`, extend `AppSettings`:

```ts
largeFileThresholdMiB: number;
largeFileStreams: number;
```

Defaults:

```ts
largeFileThresholdMiB: 20,
largeFileStreams: 8,
```

Add clamp helpers:

```ts
export function clampLargeFileThresholdMiB(value: number): number {
  if (!Number.isFinite(value)) return defaultAppSettings.largeFileThresholdMiB;
  return Math.min(4096, Math.max(1, Math.trunc(value)));
}

export function clampLargeFileStreams(value: number): number {
  if (!Number.isFinite(value)) return defaultAppSettings.largeFileStreams;
  return Math.min(16, Math.max(1, Math.trunc(value)));
}
```

Merge them in `mergeAppSettings`.

In Rust `DownloadRequest` and `AppSettings`, add:

```rust
pub large_file_threshold_mib: u16,
pub large_file_streams: u16,
```

Default to 20 and 8. Validate ranges in request/settings validation.

- [ ] **Step 6: Implement copyto argument construction and transfer selection**

Add:

```rust
fn large_file_threshold_bytes(request: &DownloadRequest) -> u64 {
    u64::from(request.large_file_threshold_mib) * 1024 * 1024
}
```

Add `build_rclone_copyto_args(remote, bucket, record, temp_path, streams)` returning a `Vec<OsString>` or argument type that preserves Unicode paths safely without shell strings.

Refactor the non-empty download branch:

- if `record.size >= large_file_threshold_bytes(request)`, run `rclone copyto` with stdout/stderr captured.
- otherwise keep the existing `cat` streaming path.
- after either path, call `verify_downloaded_file` and `install_verified_download`.
- on cancellation or failure, remove temp file.

- [ ] **Step 7: Wire frontend controls and requests**

In `App.tsx`:

- Add numeric settings controls labelled `大文件阈值` and `大文件线程`.
- Capture input values before functional state updates.
- Include `largeFileThresholdMiB` and `largeFileStreams` in `start_download` request.
- Update command preview to mention `copyto>=<threshold>MiB` for multi-file selections.

- [ ] **Step 8: Verify Task 1 green**

Run:

```powershell
npm test -- src/manifest.test.ts src/App.test.tsx
npm test
npm run build
cd src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

Expected: all pass.

- [ ] **Step 9: Commit Task 1**

```powershell
git add README.md src/manifest.ts src/manifest.test.ts src/App.tsx src/App.test.tsx src-tauri/src/manifest.rs
git commit -m "feat: add large file copyto downloads"
```

### Task 2: Stable Downloads View

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Modify: `src/App.test.tsx`
- Modify: `src-tauri/src/manifest.rs`
- Modify: `README.md`

- [ ] **Step 1: Write failing queue/UI tests**

Add frontend tests requiring:

- `下载` view button/tab exists.
- Starting a two-file download creates two stable rows before terminal events arrive.
- A `finished` event for the second file updates the existing second row and does not move it above the first row.
- Resources view displays summary text like `0 / 2` and does not display the full result message list.
- Downloads view displays full row details and `重试失败`.

- [ ] **Step 2: Run tests to verify red**

Run:

```powershell
npm test -- src/App.test.tsx
```

Expected: fails because the downloads view and queued row model do not exist.

- [ ] **Step 3: Add backend queued events**

In `download_records_with_progress`, emit one `queued` event per deduped record after the batch-level queued event:

```rust
progress_sink.emit(download_progress_event(
    task_id,
    "queued",
    Some(&record.path),
    0,
    record.size,
    0,
    0,
    total_files,
    &format!("queued {}", record.path),
));
```

Keep the existing batch `queued` event with `path: None`.

- [ ] **Step 4: Implement frontend queue model**

In `App.tsx`, replace append-only `downloadResults` usage with stable queue entries:

```ts
type DownloadQueueItem = DownloadItemResult & {
  bytesWritten: number;
  totalBytes: number;
};
```

When `downloadPaths(paths)` starts, initialize rows from `paths`. On progress events with a path, update the matching row in place. Keep row order from selected paths.

- [ ] **Step 5: Add resources/downloads view switch**

Add state:

```ts
const [activeView, setActiveView] = useState<"resources" | "downloads">("resources");
```

Render a compact segmented control in the main content header. Resources view renders the current table and compact overall progress panel. Downloads view renders queue rows, status, message, byte progress, cancel, retry failed, and open folder actions.

- [ ] **Step 6: Verify Task 2 green**

Run:

```powershell
npm test -- src/App.test.tsx
npm test
npm run build
cd src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

Expected: all pass.

- [ ] **Step 7: Capture visual evidence**

Run the Vite mock or Tauri dev visual capture used by previous `.agent/visual` evidence. Capture:

- resources view summary progress
- downloads view stable queue

Write `.agent/visual/ebook-neo-downloads-page.md` with changed files, URL/window, viewport, artifact filenames, and observed result.

- [ ] **Step 8: Commit Task 2**

```powershell
git add README.md src/App.tsx src/App.css src/App.test.tsx src-tauri/src/manifest.rs .agent/visual
git commit -m "feat: add stable downloads view"
```

### Task 3: Final Verification And Documentation Sweep

**Files:**
- Modify: `memory/progress.md`
- Modify: `memory/verify.md`

- [ ] **Step 1: Run full quality gate**

```powershell
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
cd src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
cd ..
git diff --check
```

- [ ] **Step 2: Update memory files**

Record the completed commits, verification commands, and any remaining known limitations:

- `copyto` progress shows active row but not exact live byte progress in this slice.
- R2/route speed is still the larger bottleneck if throughput remains low.

- [ ] **Step 3: Commit memory if appropriate**

Memory files live outside the repo and are not committed. Confirm `ebook-neo-desktop` git status is clean except intended local commits.
