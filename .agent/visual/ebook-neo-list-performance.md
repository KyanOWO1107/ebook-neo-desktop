# Ebook Neo List Performance Visual Evidence

- Changed files: src/App.tsx, src/App.css, src/VirtualList.tsx, src/manifest.ts, README.md
- Route: http://127.0.0.1:1423/ with Tauri invoke/event mocks
- Viewport: 1180x760
- Artifacts:
  - ebook-neo-list-performance-resources.png
  - ebook-neo-list-performance-downloads.png
  - ebook-neo-list-performance-sync.png
- Observed result: resources page shows 1,000 mock records through a virtualized table; downloads page shows a stable virtualized queue after selecting 1,000 files; sync page shows virtualized pending and extra lists for 800 mock rows each.
- Fit/DOM metrics: {"resourceMetrics":{"total":"1000","renderedRows":20,"box":{"x":281,"y":300.3125,"width":534,"height":438.6875,"top":300.3125,"right":815,"bottom":739,"left":281}},"downloadMetrics":{"total":"1000","renderedRows":17,"visibleLastRow":false,"box":{"x":281,"y":249.828125,"width":534,"height":489.171875,"top":249.828125,"right":815,"bottom":739,"left":281}},"syncMetrics":{"pendingTotal":"800","extraTotal":"800","renderedRows":30,"visibleLastPending":false,"pendingBox":{"x":295,"y":357.578125,"width":283.90625,"height":367.421875,"top":357.578125,"right":578.90625,"bottom":725,"left":295},"extraBox":{"x":590.90625,"y":347.140625,"width":210.09375,"height":377.859375,"top":347.140625,"right":801,"bottom":725,"left":590.90625}}}
- Console errors/warnings: 0

