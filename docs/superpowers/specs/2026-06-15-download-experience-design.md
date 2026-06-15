# Download Experience Design

## Scope

This slice improves collaborator downloads without changing the maintainer upload workflow or exposing R2 publicly.

Included:

- A short collaborator quickstart document.
- A GUI action that checks whether the configured `rclone` remote can read the configured R2 bucket.
- A GUI action that opens the configured download directory.
- Directory-level selection for the active folder.
- Structured per-file download results in the GUI.
- A retry action for files that failed in the last download run.

Deferred:

- Live byte progress while each file is streaming.
- Packaging installers for Windows, macOS, and Linux.
- Upload or manifest-generation workflows in the GUI.

## User Flow

Collaborators clone the neo index repository, configure a read-only R2 remote in rclone, start the desktop client, check connectivity, update the resource table, search or select a folder, download selected files, and retry failed files if needed.

The GUI remains dense and practical: the resource table stays central, the right panel becomes a small operations panel with settings, queue preview, result rows, and action buttons.

## Backend Shape

The Rust backend keeps direct `rclone cat` downloads. It will return structured results:

- `path`
- `status`: `downloaded`, `createdEmpty`, or `failed`
- `message`

Batch downloads should keep going after a single file fails. The GUI can then retry only failed paths. The backend still verifies file size and `sha256` before installing the final file.

The rclone check uses a low-cost read operation against the configured bucket, such as `rclone lsf <remote>:<bucket> --max-depth 1`, and reports stdout/stderr without mutating remote data.

Opening the download directory is handled through the existing Tauri opener plugin from the frontend after the directory path is resolved or created by the backend.

## Frontend Shape

The right panel adds:

- Check rclone button.
- Open download directory button.
- Select active folder button near the table actions.
- Download result rows with file name, status, and short message.
- Retry failed button when the last run has failures.

The current command preview remains useful for advanced users, but successful and failed files no longer need to be inferred from one large text block.

## Testing

Use TDD for behavior changes.

Required checks:

- Frontend helper tests for directory selection and retry state.
- Frontend component tests for settings inputs and new action buttons.
- Rust tests for rclone check arguments, structured result reporting, per-file failure continuation, and download directory resolution.
- `npm test`
- `npm run build`
- `cargo test`
- `cargo check`
- Visual screenshot evidence after UI changes.

