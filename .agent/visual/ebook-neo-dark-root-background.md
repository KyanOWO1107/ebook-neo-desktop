# Dark Root Background Visual Evidence

- Changed files: `src/App.tsx`, `src/App.css`, `src/App.test.tsx`, `.agent/visual/ebook-neo-dark-root-background.png`
- Route or URL: `http://127.0.0.1:1425/` with Tauri invoke/event mocks returning saved dark theme settings.
- Viewport: `1180x520`.
- Artifact filename: `ebook-neo-dark-root-background.png`.
- Observed result: the document root and app both use `data-theme="dark"`; computed background colors for `html`, `body`, `#root`, and `.app-shell` are all `rgb(11, 19, 36)`. The test viewport was shorter than the page minimum height (`520` vs `640`) to expose the underlying page background. Browser console/page errors and warnings: 0.
