# Ebook Neo Downloads Page Visual Evidence

- Changed files: `src/App.tsx`, `src/App.css`, `src/App.test.tsx`, `src-tauri/src/manifest.rs`, `README.md`
- Route or URL: `http://127.0.0.1:1421/`
- Viewport: `1180x760`
- Artifact filenames: `ebook-neo-downloads-page-resources.png`, `ebook-neo-downloads-page-queue.png`
- Observed result: the resources view shows only compact overall download progress (`0 / 3`) while keeping the resource table visible. The downloads view shows a stable per-file queue with all selected rows present; after a finished event for `资料/数据结构/b.pdf`, that row updates in place to `完成` with its message while the original order remains unchanged. Browser console errors and warnings were 0 in the captured run.
