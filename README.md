# Ebook Neo Desktop

这是 `TYUT-ebooks-collection-neo` 的独立跨平台桌面客户端，技术栈为 Tauri v2、React、TypeScript 和 Vite。

当前 MVP 用于协作者私有下载：

- 读取仓库中的 `manifests/files.jsonl`。
- 按根目录浏览资料统计。
- 搜索、勾选单个文件或当前列表。
- 通过内置 Tauri 后端调用 `rclone cat` 从 Cloudflare R2 只读 remote 下载。
- 保存索引仓库、下载目录、rclone、R2 remote/bucket、下载并发数和亮/暗主题设置。
- 通过 `git pull --ff-only` 从 GitHub 更新资源表，然后重新加载 manifest。

客户端本身作为独立仓库维护。它通过“索引仓库”设置指向本地的 `TYUT-ebooks-collection-neo` 克隆目录，只从该目录读取资源表并执行 Git 更新；下载不再依赖索引仓库中的 Python 工具。

当前版本不包含上传、账号管理或公开下载功能。上传和 manifest 生成仍由维护者在本地命令行完成，并通过 Git/GitHub 推送。

协作者从零开始配置和下载资料，可先看索引仓库中的短指南：

```text
TYUT-ebooks-collection-neo/docs/collaborator-quickstart.md
```

## 前置条件

所有平台都需要：

- Node.js `20.19+` 或 `22.12+`，以及 npm。
- Rust 和 Cargo。
- rclone，并且已经配置只读 R2 remote。
- 本地克隆 `TYUT-ebooks-collection-neo` 索引仓库。

默认下载设置：

```text
index repo: ../TYUT-ebooks-collection-neo
download root: downloads/gui
rclone: rclone
remote: ebookneo-r2-readonly
bucket: tyut-ebooks-collection-neo
jobs: 4
large file threshold: 20 MiB
large file streams: 8
theme: light
```

如果你的 GUI 仓库和索引仓库不是并排目录，请在应用右侧设置里把“索引仓库”改成绝对路径，例如：

```text
path\to\TYUT-ebooks-collection-neo
```

如果 Windows 上 `rclone` 没有进入 PATH，可以在应用右侧设置里填完整路径，例如：

```text
your-rclone-path\rclone.exe
```

Linux/macOS 协作者通常可以保持 `rclone`。如果没有加入 PATH，也可以填写绝对路径，例如 `/usr/local/bin/rclone`。应用会校验这个路径的文件名必须是 `rclone` 或 `rclone.exe`。

## 开发命令

在本目录运行：

```bash
npm install
npm run tauri dev
```

常用验证：

```bash
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
```

Rust 后端验证在 `src-tauri` 中运行：

```bash
cargo fmt --check
cargo test
cargo check
```

如果 Windows 上并行编译占用内存过高，可以限制 Cargo 并发：

```powershell
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

仓库包含 GitHub Actions CI。推送到 `main` 或创建面向 `main` 的 Pull Request 时，CI 会在 Ubuntu 上执行上述前端和 Rust 检查；它只做质量门禁，不生成安装包。

## 发布流程

Windows 和 Linux 安装包由 `.github/workflows/release.yml` 生成。这个 workflow 可以在 GitHub Actions 页面手动运行，也可以通过推送版本 tag 触发。

手动运行时填写版本号，例如：

```text
0.1.0
```

Tag 触发时使用：

```bash
git tag ebook-neo-desktop-v0.1.0
git push origin ebook-neo-desktop-v0.1.0
```

Release 会先创建为 draft，确认产物和说明后再手动发布。当前只构建：

- Windows: `.msi` 和 NSIS `.exe`
- Linux: `.deb` 和 `.AppImage`

macOS 暂不包含在发布矩阵中。当前构建也不做代码签名，Windows 用户可能看到 SmartScreen 提示。Linux AppImage 首次运行前可能需要添加执行权限：

```bash
chmod a+x Ebook*.AppImage
```

发布包仍要求协作者自行安装并配置 `rclone` 只读 R2 remote；应用不会内置 R2 token 或 rclone 配置。

## 下载行为

点击“开始下载”后，应用会在“索引仓库”目录执行等价命令：

```bash
rclone cat ebookneo-r2-readonly:tyut-ebooks-collection-neo/<object_key> > downloads/gui/<manifest-path>.ebook-neo-part
rclone copyto ebookneo-r2-readonly:tyut-ebooks-collection-neo/<object_key> downloads/gui/<manifest-path>.ebook-neo-part --multi-thread-streams 8 --multi-thread-cutoff 1M --multi-thread-chunk-size 16M
```

应用会先从 `manifests/files.jsonl` 找到选中文件对应的 `object_key`、`size` 和 `sha256`。小文件使用 `rclone cat` 流式写入临时文件；达到“大文件阈值”的文件使用 `rclone copyto` 写入同一个临时文件，并启用 rclone 的多线程下载参数。写完后会校验文件大小和 `sha256`，校验通过才替换到目标路径。

再次下载同一文件时，会写入同一个目标路径并重新校验，不会在旁边生成重复副本。

`并发`设置控制 GUI 后端同时下载的文件数量，范围为 1 到 16。默认值是 4，网络或机器压力较大时可以调低到 1 或 2。

`大文件阈值`控制何时从 `cat` 切换为 `copyto` 多线程路径，单位为 MiB，默认值是 20。`大文件线程`控制单个大文件的 rclone 多线程流数量，默认值是 8，范围为 1 到 16。它和文件级 `并发` 是两个不同设置：前者加速单个大文件，后者控制同时下载多少个文件。

下载完成后，右侧面板会显示逐文件结果。单个文件失败不会阻止整批任务继续，失败项会保留在结果列表中，可点击“重试失败”只重试这些路径。

下载过程中，右侧面板会显示当前文件的字节级进度、整体完成数量和取消按钮。`cat` 路径会显示实时字节进度；`copyto` 大文件路径当前显示开始、完成或失败状态。取消会停止后续排队文件，并让后端尽量终止当前 rclone 进程；已经写入但未校验完成的 `.ebook-neo-part` 临时文件会被清理。

“检查 R2”会运行只读的 rclone 列目录检查，用于确认当前 `rclone`、Remote 和 Bucket 设置是否可用。“打开目录”会创建并打开当前下载目录。

## 更新资源表

点击“更新资源表”后，应用会在“索引仓库”目录运行：

```bash
git pull --ff-only
```

如果本地有未提交修改、分支无法快进或发生冲突，Git 会正常失败，应用会展示 stdout/stderr，不会自动覆盖本地文件。

## 已知边界

- 选择粒度支持文件、当前可见列表和当前目录。
- 更新 R2 对象和生成 manifest 仍使用命令行工具，暂不放入 GUI。
