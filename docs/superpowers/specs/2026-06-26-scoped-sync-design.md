# Scoped Sync Design

## Context

The desktop app already supports read-only collaborator sync against a saved `syncRoot`. The current scan always compares the full manifest, which can be slow and noisy for collaborators who only need one course folder.

## Design

Add a sync scope control to the `同步` view with two modes:

- `全部资料`: scan the full manifest, matching current behavior.
- `当前目录`: scan only the folder selected in the left sidebar, using the same manifest path prefix semantics as folder selection and download selection.

When `当前目录` is selected and no sidebar folder is active, scanning is disabled and the view explains that a folder must be selected first. The backend receives an optional `scopePrefix` in `scan_sync_plan`; if provided, it filters manifest records to exact prefix boundary matches before comparing local files. Extra local files are still reported relative to the whole sync folder, but scan counts and pending downloads are only for scoped manifest records.

Add an `打开同步目录` action that prepares and opens the saved `syncRoot`. It reuses the existing Rust-side directory preparation/opening path so the frontend does not gain direct filesystem opener permissions.

## Error Handling

- Invalid or empty scope prefixes are rejected by the same manifest path validation used for manifest records.
- A scope prefix that matches no records returns an empty plan with normal zero counts.
- Opening the sync folder surfaces the backend error in the existing status/log area.

## Tests

- Rust: `scan_sync_plan` filters by `scopePrefix` and rejects invalid prefixes.
- Frontend: scoped sync sends the selected folder prefix, starts downloads only for scoped paths, and `打开同步目录` invokes the directory opener with `syncRoot`.
