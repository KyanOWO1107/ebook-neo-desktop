# Large File Progress And Settings Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add configurable large-file progress display and move persistent configuration into a dedicated settings view.

**Architecture:** Rust keeps owning transfer execution and settings persistence. Large `copyto` downloads optionally emit progress by polling `.ebook-neo-part` size. React adds a `设置` view and moves long-lived inputs out of the right download panel while preserving workflow controls there.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, Vite, Vitest, Testing Library, rclone.

---

### Task 1: Backend Large-File Progress Setting

**Files:**
- Modify: `src-tauri/src/manifest.rs`

- [ ] **Step 1: Write failing Rust tests**

Add tests near the existing large-file download tests:

```rust
#[test]
fn copyto_large_file_emits_temp_file_progress_when_enabled() {
    // Use a fake rclone that writes a partial file, sleeps, then finishes.
    // Expect at least one `progress` event with bytes_written > 0 and < record.size.
}

#[test]
fn copyto_large_file_skips_temp_file_progress_when_disabled() {
    // Use the same fake rclone and show_large_file_progress: false.
    // Expect no `progress` events for the large file before the final `finished` event.
}
```

Also update default/settings serde tests to require `show_large_file_progress == true` and JSON field `showLargeFileProgress`.

- [ ] **Step 2: Run Rust tests to verify red**

```powershell
cd src-tauri
$env:CARGO_BUILD_JOBS='1'; cargo test large_file_progress -- --nocapture
```

Expected: fails because the field and progress polling do not exist.

- [ ] **Step 3: Implement backend setting and progress polling**

Add `show_large_file_progress: bool` to `DownloadRequest` and `AppSettings`, defaulting to true. Include it in validation tests and request constructors.

In `run_rclone_copyto_download`, inside the existing polling loop:

```rust
if request.show_large_file_progress {
    if let Ok(metadata) = fs::metadata(temp_path) {
        let bytes_written = metadata.len().min(record.size);
        progress_sink.emit(download_progress_event(
            task_id,
            "progress",
            Some(&record.path),
            bytes_written,
            record.size,
            completed_files,
            failed_files,
            total_files,
            &format!("copying {}", record.path),
        ));
    }
}
```

Throttle to about once per second using `Instant`.

- [ ] **Step 4: Verify backend green**

```powershell
cd src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test large_file_progress -- --nocapture
$env:CARGO_BUILD_JOBS='1'; cargo test
```

- [ ] **Step 5: Commit backend slice**

```powershell
git add src-tauri/src/manifest.rs
git commit -m "feat: add optional large file progress polling"
```

### Task 2: Frontend Setting And Request Payload

**Files:**
- Modify: `src/manifest.ts`
- Modify: `src/manifest.test.ts`
- Modify: `src/App.test.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write failing frontend helper tests**

In `src/manifest.test.ts`, require:

```ts
expect(defaultAppSettings.showLargeFileProgress).toBe(true);
expect(mergeAppSettings({ showLargeFileProgress: false })).toMatchObject({
  showLargeFileProgress: false,
});
expect(buildDownloadRequestPayload(defaultAppSettings, ["资料/a.pdf"])).toMatchObject({
  showLargeFileProgress: true,
});
```

- [ ] **Step 2: Run helper tests to verify red**

```powershell
npm test -- src/manifest.test.ts
```

Expected: fails because the field does not exist.

- [ ] **Step 3: Implement frontend settings field**

Add `showLargeFileProgress: boolean` to `AppSettings` and `DownloadRequestPayload`, default true, merged as a boolean.

- [ ] **Step 4: Verify frontend helper green**

```powershell
npm test -- src/manifest.test.ts
```

- [ ] **Step 5: Commit frontend payload slice**

```powershell
git add src/manifest.ts src/manifest.test.ts
git commit -m "feat: add large file progress setting"
```

### Task 3: Settings View UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing UI tests**

In `src/App.test.tsx`, add tests requiring:

- a `设置` tab exists
- clicking `设置` reveals `索引仓库`, `下载目录`, `rclone`, `Remote`, `Bucket`, `并发`, `大文件阈值`, `大文件线程`, `大文件下载进度展示`, and `保存设置`
- the right download panel no longer contains the full settings form when `资料` is active
- toggling `大文件下载进度展示` and starting a download sends `showLargeFileProgress: false`

- [ ] **Step 2: Run UI tests to verify red**

```powershell
npm test -- src/App.test.tsx
```

Expected: fails because the settings tab and toggle do not exist.

- [ ] **Step 3: Implement settings view**

Change `activeView` to `"resources" | "downloads" | "settings"`. Add a third tab. Move the settings grid into a settings view. Use a checkbox/toggle for `大文件下载进度展示`.

Keep the right panel focused on queue, log, progress, utility actions, cancel, and start download.

- [ ] **Step 4: Verify UI green**

```powershell
npm test -- src/App.test.tsx
npm test
npm run build
```

- [ ] **Step 5: Commit settings UI slice**

```powershell
git add src/App.tsx src/App.css src/App.test.tsx
git commit -m "feat: move download configuration to settings view"
```

### Task 4: Docs, Visual Evidence, And Final Verification

**Files:**
- Modify: `README.md`
- Add: `.agent/visual/ebook-neo-settings-view.md`
- Add: `.agent/visual/ebook-neo-settings-view.png`
- Add: `.agent/visual/ebook-neo-simplified-download-panel.png`
- Modify: `memory/progress.md`
- Modify: `memory/verify.md`

- [ ] **Step 1: Update README**

Document:

- `大文件下载进度展示`
- the `设置` page
- large-file progress uses temp-file size polling when enabled

- [ ] **Step 2: Capture visual evidence**

Run the Vite mock used by prior visual checks. Capture:

- settings view with moved controls
- resources/downloads view with simplified right panel

- [ ] **Step 3: Run final quality gate**

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

- [ ] **Step 4: Update memory files**

Record completed commits, verification output, and any known limitation.

- [ ] **Step 5: Commit docs/evidence slice**

```powershell
git add README.md
git add -f .agent/visual/ebook-neo-settings-view.md .agent/visual/ebook-neo-settings-view.png .agent/visual/ebook-neo-simplified-download-panel.png
git commit -m "docs: document settings view and large file progress"
```
