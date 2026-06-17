# Large File Progress And Settings Page Design

## Goal

Add an optional large-file progress display for `rclone copyto` downloads and move persistent configuration into a dedicated settings view.

## Requirements

- Add a saved setting named `showLargeFileProgress`.
- Default `showLargeFileProgress` to enabled.
- When enabled, large-file downloads using `rclone copyto` emit progress events by reading the `.ebook-neo-part` temporary file size and comparing it with the manifest record size.
- When disabled, large-file downloads still show `下载中`, but the byte display stays in a non-numeric waiting state until the file finishes.
- Keep existing `cat` small-file byte progress unchanged.
- Add a `设置` view alongside `资料` and `下载`.
- Move long-lived configuration from the right download panel to the settings view:
  - index repository path
  - download root
  - rclone path
  - R2 remote
  - bucket
  - file download concurrency
  - large-file threshold
  - large-file streams
  - large-file progress toggle
  - theme
- Keep the right download panel focused on active workflow:
  - selected queue
  - command/status log
  - current progress
  - cancel/start actions
  - check R2
  - open download directory
  - save settings shortcut if settings were edited

## Backend Design

`DownloadRequest` and `AppSettings` gain a boolean field:

```rust
pub show_large_file_progress: bool
```

The serde name is `showLargeFileProgress`. The backend validates no extra range because it is a boolean. In `run_rclone_copyto_download`, the existing process polling loop reads `fs::metadata(temp_path).len()` at a throttled interval when the setting is enabled. Each successful read emits a `progress` event with `bytes_written` clamped to `record.size`. Missing temp files or transient metadata errors are ignored.

This keeps `rclone copyto` as the transfer engine and avoids parsing rclone's text output. Verification and final install stay unchanged: size and SHA256 are checked after rclone exits, then the temp file is renamed into place.

## Frontend Design

The main tab control becomes:

```text
资料 | 下载 | 设置
```

The settings view uses compact grouped sections:

- Paths: index repository, download directory, rclone
- R2: remote, bucket, check R2 action
- Downloads: concurrency, large-file threshold, large-file streams, large-file progress toggle
- Appearance: light/dark theme toggle

The right panel removes the dense settings grid. It keeps workflow actions and progress. Download request construction includes `showLargeFileProgress`.

When progress events arrive with bytes for a copyto file, the existing queue rows update the same way as small-file rows. When the toggle is disabled, no intermediate `progress` events are emitted for copyto, so rows stay `下载中` with `等待进度` until `finished`.

## Testing

- Frontend helper tests verify the default setting, merge behavior, request payload field, and toggling behavior.
- Frontend UI tests verify the `设置` view exists and contains the moved controls, while the download panel no longer exposes the full settings grid.
- Rust tests verify:
  - default settings include `show_large_file_progress: true`
  - `DownloadRequest` deserializes `showLargeFileProgress`
  - copyto emits intermediate progress from temp-file size when enabled
  - copyto does not emit intermediate progress when disabled

## Visual Evidence

Capture a settings view screenshot and a download view screenshot showing the simplified right panel.
