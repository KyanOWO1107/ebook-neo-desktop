# List Performance And UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep resource browsing, downloads, and sync views responsive with large lists while improving queue and sync usability.

**Architecture:** Add a local fixed-row virtual list component and move repeated list/index helpers into small tested TypeScript utilities. Keep backend download behavior unchanged; frontend state updates become more targeted and progress events are batched before rendering.

**Tech Stack:** Tauri v2, React 19, TypeScript, Vitest, Testing Library, Rust tests for unchanged backend command registration.

---

### Task 1: Virtual List Foundation

**Files:**
- Create: `src/VirtualList.tsx`
- Create: `src/VirtualList.test.tsx`
- Modify: `src/App.css`

- [ ] Write tests that render 1,000 virtual rows and assert only a bounded subset is present.
- [ ] Verify the tests fail before creating the component.
- [ ] Implement a fixed-row virtual list with `itemCount`, `rowHeight`, `overscan`, `className`, and `renderRow`.
- [ ] Add CSS helpers for virtual list containers and rows.
- [ ] Run `npm test -- src/VirtualList.test.tsx`.
- [ ] Commit the foundation slice.

### Task 2: Manifest And Queue Helpers

**Files:**
- Modify: `src/manifest.ts`
- Modify: `src/manifest.test.ts`

- [ ] Add failing tests for `buildRecordIndex`, `selectedRecordsFromIndex`, `initializeDownloadQueue`, `updateDownloadQueueItem`, and `filterDownloadQueue`.
- [ ] Verify the tests fail for missing exports.
- [ ] Implement the helpers without changing app behavior.
- [ ] Run `npm test -- src/manifest.test.ts`.
- [ ] Commit the helper slice.

### Task 3: Resource And Downloads Virtualization

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`

- [ ] Add failing tests requiring resource results above 500 to be accessible through virtualized rendering.
- [ ] Add failing tests requiring a large download queue to render only visible rows while preserving total counts.
- [ ] Replace `visibleRecords.slice(0, 500)` with full filtered records rendered through `VirtualList`.
- [ ] Use the record index helper for selected records, command previews, and queue initialization.
- [ ] Add download queue filters for all, active, failed, and completed.
- [ ] Run `npm test -- src/App.test.tsx src/manifest.test.ts src/VirtualList.test.tsx`.
- [ ] Commit the resource/download virtualization slice.

### Task 4: Progress Batching And Sync UX

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/App.css`

- [ ] Add failing tests showing burst progress events update the final row state without appending or reordering entries.
- [ ] Add failing tests for sync pending filters and virtualized sync rows.
- [ ] Batch path-level progress events with `requestAnimationFrame`.
- [ ] Flush completed events promptly so buttons unlock after a task ends.
- [ ] Virtualize sync pending and extra lists.
- [ ] Add sync pending filters for all, missing, and outdated.
- [ ] Run targeted frontend tests.
- [ ] Commit the progress/sync UX slice.

### Task 5: Verification, Visual Evidence, And Docs

**Files:**
- Modify: `README.md`
- Add: `.agent/visual/ebook-neo-list-performance-*.png`
- Add: `.agent/visual/ebook-neo-list-performance.md`
- Modify: `memory/progress.md`
- Modify: `memory/verify.md`

- [ ] Update README with performance behavior and list filter notes.
- [ ] Run `npm test`.
- [ ] Run `npm run build`.
- [ ] Run `cargo fmt --check` in `src-tauri`.
- [ ] Run `$env:CARGO_BUILD_JOBS='1'; cargo test` in `src-tauri`.
- [ ] Run `$env:CARGO_BUILD_JOBS='1'; cargo check` in `src-tauri`.
- [ ] Run `git diff --check`.
- [ ] Capture visual evidence for resource, downloads, and sync views.
- [ ] Commit final docs/evidence slice.
