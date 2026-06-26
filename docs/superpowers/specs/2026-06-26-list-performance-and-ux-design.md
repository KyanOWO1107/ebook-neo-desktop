# List Performance And UX Design

## Context

The desktop client now supports resource browsing, direct downloads, a downloads page, and a read-only sync page. The manifest currently has more than 11,000 records. Resource browsing limits rendered rows to 500, but the downloads page renders every queued file. Large sync operations can enqueue hundreds or thousands of files, and each progress event currently maps over the whole queue before React renders the visible page again.

This creates two user-visible problems:

- The downloads page can feel sluggish after starting a sync with many files.
- The resource page can feel intermittently slow because download progress, selection, search, and visible rows all live in one large component.

## Goals

- Keep the app responsive when a download or sync queue contains thousands of files.
- Let users browse the full matching resource list without the current 500-row hard cutoff.
- Keep the downloads page useful by showing the full queue through virtual scrolling rather than hiding most items.
- Improve sync and download usability with compact filters, summary text, and clearer target-folder information.
- Keep this slice frontend-focused; do not change R2 object layout, credentials, or download verification behavior.

## Non-Goals

- Do not introduce local deletion, quarantine, or automatic cleanup of extra sync files.
- Do not change the Rust download protocol or R2 access model.
- Do not add a new external virtualization dependency unless the local implementation becomes insufficient.
- Do not redesign the app navigation or color system in this slice.

## Architecture

Add a small reusable fixed-row virtual list component in React. It receives an item count, row height, overscan, and a render callback. It renders only the rows near the current scroll position while preserving the correct total scroll height.

Use the virtual list in:

- Resource rows.
- Downloads queue rows.
- Sync pending rows.
- Sync extra-file rows.

Add lightweight frontend helpers for performance-sensitive state:

- A manifest index helper that maps `path -> record`, avoiding repeated full scans when building command previews or initial queue rows.
- A selected-record helper that resolves selected paths through the index instead of filtering every manifest record.
- A queue update helper that updates one path while preserving row order.
- A requestAnimationFrame based progress flush so bursty download progress events are batched before React state updates.

## UX Changes

Resource view:

- Show the full filtered match count.
- Render matching records through virtual scrolling instead of slicing to 500.
- Keep "选择当前列表" scoped to the visible filtered result set.

Downloads view:

- Keep the full queue, but virtualize visible rows.
- Add a compact filter: all, active, failed, completed.
- Preserve the current retry-failed and open-folder actions.
- Show target folder and file count in the view header.

Sync view:

- Keep scan output read-only.
- Add a compact filter for pending items: all pending statuses, missing, outdated.
- Virtualize pending and extra-file lists.
- Keep the "extra local files are only shown, never deleted" behavior clear in the empty/help text.

## Error Handling

- If the virtual list is given zero items, it renders the existing empty state outside the list.
- If measured container height is unavailable, the component falls back to a safe fixed height through CSS.
- Progress event batching must flush the latest event for each path and still handle the final `completed` event promptly.

## Testing

Frontend tests:

- Virtual list renders only a bounded number of rows while preserving a large total item count.
- Resource search can report and scroll a result set larger than 500 rows.
- Downloads queue initializes thousands of rows without rendering all of them.
- Download progress updates the matching row without reordering queue entries.
- Download filters show active, failed, and completed subsets.
- Sync filters show pending and extra lists without rendering every row at once.

Verification:

- `npm test`
- `npm run build`
- `cargo fmt --check`
- `cargo test`
- `cargo check`
- `git diff --check`
- Fresh visual evidence for resource, downloads, and sync views with large mocked lists.
