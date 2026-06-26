# Scoped Sync Visual Evidence

- Changed files: `src/App.tsx`, `src/App.css`, `src/App.test.tsx`, `README.md`, `docs/superpowers/plans/2026-06-26-scoped-sync.md`, `.agent/visual/ebook-neo-scoped-sync.png`
- Route or URL: `http://127.0.0.1:1424/` with Tauri invoke/event mocks.
- Viewport: `1180x760`.
- Artifact filename: `ebook-neo-scoped-sync.png`.
- Observed result: the `同步` view shows the `同步范围` control set to `当前目录`, reports `当前目录：资料/数据结构`, scans only that folder with `scopePrefix: "资料/数据结构"`, lists one missing pending file, and `打开同步目录` invokes the safe open-directory command with `downloadRoot: "downloads/sync"`. Browser console/page errors and warnings: 0.
