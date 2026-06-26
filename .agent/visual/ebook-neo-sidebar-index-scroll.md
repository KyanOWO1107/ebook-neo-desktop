# Sidebar Index Scroll Visual Evidence

- Changed files: `src/App.css`, `src/App.test.tsx`, `.agent/visual/ebook-neo-sidebar-index-scroll.png`
- Route or URL: `http://127.0.0.1:1426/` with Tauri invoke/event mocks returning 120 folder-heavy manifest records.
- Viewport: `1180x640`.
- Artifact filename: `ebook-neo-sidebar-index-scroll.png`.
- Observed result: the sidebar brand/title and `全部资料` row remain outside the scrolling container. The `.folder-list` starts below them, has `overflow-y: auto`, and is the only overflowing sidebar area (`scrollHeight 1726`, `clientHeight 478`). The sidebar itself uses `overflow: hidden`, and the body height stayed equal to the viewport (`640`), so the left index scroll no longer affects the right workspace layout. Browser console/page errors and warnings: 0.
