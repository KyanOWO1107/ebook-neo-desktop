# Live Download Progress Design

## Scope

This slice improves the existing direct R2 download flow in the independent Tauri desktop app. It keeps the maintainer upload workflow, manifest format, rclone configuration, and read-only R2 access model unchanged.

Included:

- Start downloads as a background task instead of waiting for one long `download_selected` call.
- Emit live progress events while `rclone cat` streams bytes into `.ebook-neo-part`.
- Show overall progress, current file progress, completed/failed counts, and recent per-file status in the GUI.
- Add a cancel action that stops queued work, terminates active rclone child processes when possible, removes partial files, and reports canceled files.
- Keep the existing `download_selected` command as a compatibility path and regression-test surface.

Deferred:

- Pause/resume or byte-range continuation.
- Persisting download sessions across app restarts.
- Packaging installers, bundled rclone, signing, or auto-update.

## Backend Design

Add two Tauri commands:

- `start_download(request: DownloadRequest) -> Result<DownloadTask, String>`
- `cancel_download(taskId: String) -> Result<CommandResult, String>`

`start_download` validates the request, resolves selected records, creates a unique task id, stores a cancel flag in process state, and spawns a blocking worker. The worker uses the existing direct `rclone cat` path and existing destination safety checks.

Progress is emitted through Tauri events named `download-progress`. Every event includes:

- `taskId`
- `kind`: `queued`, `started`, `progress`, `finished`, `failed`, `canceled`, or `completed`
- `path`
- `bytesWritten`
- `totalBytes`
- `completedFiles`
- `failedFiles`
- `totalFiles`
- `message`

For non-empty files, the copy loop reads stdout in chunks and increments `bytesWritten` after each successful write. For zero-byte files, the backend emits a completed item immediately after the temp-file install path succeeds.

Cancellation uses a shared `Arc<AtomicBool>` per task. Queue workers check the flag before starting the next file. Active file downloads check it inside the streaming loop; if set, they kill the rclone child, remove the temp file, and return a `canceled` item result. A final `completed` event is still emitted with a cancellation message so the frontend can settle state.

## Frontend Design

The React app subscribes to `download-progress` once on mount. It ignores events whose `taskId` does not match the current task.

When the user clicks “开始下载”:

1. Clear previous live progress and results.
2. Call `start_download`.
3. Store the returned `taskId`.
4. Keep the UI interactive while progress events update state.

While a task is active, show:

- Overall progress bar based on completed/failed/canceled files over total files.
- Current file name and byte progress when available.
- A cancel button next to the primary download action.
- Result rows for downloaded, failed, created-empty, and canceled items.

The existing retry action continues to retry only failed paths, not canceled paths. If the user cancels and wants those files later, the normal selection remains available.

## Testing

Use TDD for each behavior slice.

Backend required tests:

- `start_download` returns a task id and runs as an async command.
- progress event payload construction reports file bytes and task counts.
- streaming copy emits progress and final downloaded item result.
- cancellation removes `.ebook-neo-part` and returns a canceled item.

Frontend required tests:

- the app subscribes to `download-progress`.
- starting a download calls `start_download` and shows live progress from emitted events.
- canceling calls `cancel_download` with the active task id.
- retry failed ignores canceled items.

Verification:

- `npm test`
- `npm run build`
- `npm audit --audit-level=moderate --registry=https://registry.npmjs.org`
- `cargo fmt --check`
- `$env:CARGO_BUILD_JOBS='1'; cargo test`
- `$env:CARGO_BUILD_JOBS='1'; cargo check`
- `git diff --check`
- fresh screenshot evidence for the live progress UI
