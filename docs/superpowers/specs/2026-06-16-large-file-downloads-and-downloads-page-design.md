# Large File Downloads And Downloads Page Design

## Context

The desktop client currently downloads every non-empty file by spawning `rclone cat`, streaming stdout into a `.ebook-neo-part` file, verifying size and SHA256, then installing the verified file at the manifest path. This path is safe for Unicode destination paths and gives byte-level progress, but recent benchmarks against a real Cloudflare R2 object showed poor single-file throughput. A diagnostic `rclone copyto --multi-thread-streams 8` path improved throughput compared with `cat`, but direct R2/S3 API route speed still appears to be the larger limit.

The current download result list also updates only when terminal file events arrive. In multi-file downloads this makes the right panel feel unstable because rows appear in completion order instead of showing the whole queue.

## Goals

- Add a configurable large-file download path using `rclone copyto` with multi-thread options.
- Preserve the existing safe install flow: temp file, size check, SHA256 verification, final rename.
- Keep small-file downloads on `rclone cat` for the current streaming progress and Unicode compatibility.
- Add a dedicated downloads view that shows the full task queue with stable row order and per-file status.
- Keep the resources view focused on browsing and selection; it should show only overall download progress and file counts while a task is active.

## Non-Goals

- Do not change Cloudflare R2 bucket location or storage layout in this slice.
- Do not introduce direct S3 SDK downloads yet.
- Do not embed R2 tokens or rclone configuration in the app.
- Do not remove the existing manifest validation or SHA256 verification.

## Download Strategy

The backend will choose the transfer mode per manifest record:

- Empty files: unchanged local temp-file creation and verification.
- Files smaller than the configured threshold: use existing `rclone cat`.
- Files at or above the configured threshold: use `rclone copyto` to the generated `.ebook-neo-part` temp path.

The large-file threshold is persisted in settings as MiB. The default is `20 MiB`. The allowed range is `1` to `4096` MiB.

Large-file multi-thread stream count is persisted separately from file-level concurrency. The default is `8`. The allowed range is `1` to `16`.

The app will pass these rclone arguments for large files:

```text
copyto <remote>:<bucket>/<object_key> <temp_path>
--multi-thread-streams <large_file_streams>
--multi-thread-cutoff 1M
--multi-thread-chunk-size 16M
--stats 1s
```

If `large_file_streams` is `1`, the backend may still use `copyto` for large files but without forcing extra parallel streams. The implementation must keep argument construction shell-free.

## Progress Events

Progress events will gain stable queue information:

- `queued` events are emitted for every selected path before downloading starts.
- `started`, `progress`, `finished`, `failed`, and `canceled` keep using the selected manifest path.
- The frontend stores queue entries by path and updates existing rows instead of appending terminal events in completion order.

The first backend slice keeps byte progress exact for `cat`. For `copyto`, the backend emits `started`, then `finished` or `failed`; the UI shows the row as active while rclone runs. Parsing rclone stats for live copyto byte progress is outside this slice.

## User Interface

The app gains a simple view switch in the main content area:

- `资料` view: current browsing/search/selection table. During downloads it shows overall progress, completed/failed/canceled counts, current active file, and cancel action.
- `下载` view: stable download queue and history for the active or most recent task.

The downloads view row states are:

- `queued`
- `downloading`
- `verifying`
- `downloaded`
- `createdEmpty`
- `failed`
- `canceled`

The downloads view keeps rows in selected-path order. It shows file path, status, byte progress when available, message, and a compact error text for failures. The existing "retry failed" behavior moves to the downloads view while still using the same selected failed paths.

The right-side settings panel will add:

- `大文件阈值` numeric input in MiB, default `20`.
- `大文件线程` numeric input, default `8`.

These settings are saved and loaded with the existing settings file.

## Error Handling

- Invalid threshold or stream settings are clamped on the frontend and rejected by backend validation if malformed commands are invoked directly.
- If `copyto` fails, remove the temp file and report the failure for that item without stopping the batch.
- Cancellation must kill the active rclone process and remove the temp file for both `cat` and `copyto`.
- Verification failures after `copyto` are reported the same way as existing `cat` verification failures.

## Testing

Backend tests:

- Settings defaults include `large_file_threshold_mib = 20` and `large_file_streams = 8`.
- Backend validation accepts configured values in range and rejects out-of-range values.
- Large-file records build `rclone copyto` arguments with multi-thread settings.
- Small-file records still use `rclone cat`.
- Large-file fake-rclone download writes to the `.ebook-neo-part` destination, verifies SHA256, installs the final file, and removes the temp file.

Frontend tests:

- Settings merge defaults include the new fields.
- Settings UI renders and persists the new numeric controls.
- Starting a multi-file download initializes stable queue rows before terminal events arrive.
- Terminal progress events update existing rows instead of appending in event order.
- Resources view shows only summary progress while downloads view shows the full queue.

Verification:

- `npm test`
- `npm run build`
- `npm audit --audit-level=moderate --registry=https://registry.npmjs.org`
- `cargo fmt --check`
- `cargo test`
- `cargo check`
- visual screenshot evidence for the resources view summary and downloads view queue.
