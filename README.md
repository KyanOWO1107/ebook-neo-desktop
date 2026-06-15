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

## 前置条件

所有平台都需要：

- Node.js 和 npm。
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

Linux/macOS 协作者通常可以保持 `rclone`。如果没有加入 PATH，也可以填写绝对路径，例如 `/usr/local/bin/rclone`。

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
```

Rust 后端验证在 `src-tauri` 中运行：

```bash
cargo test
cargo check
```

如果 Windows 上并行编译占用内存过高，可以限制 Cargo 并发：

```powershell
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

## 下载行为

点击“开始下载”后，应用会在“索引仓库”目录执行等价命令：

```bash
rclone cat ebookneo-r2-readonly:tyut-ebooks-collection-neo/<object_key> > downloads/gui/<manifest-path>
```

应用会先从 `manifests/files.jsonl` 找到选中文件对应的 `object_key`、`size` 和 `sha256`，再把 `rclone cat` 的输出流式写入临时文件。写完后会校验文件大小和 `sha256`，校验通过才替换到目标路径。

再次下载同一文件时，会写入同一个目标路径并重新校验，不会在旁边生成重复副本。

`并发`设置控制 GUI 后端同时下载的文件数量，范围为 1 到 16。默认值是 4，网络或机器压力较大时可以调低到 1 或 2。

## 更新资源表

点击“更新资源表”后，应用会在“索引仓库”目录运行：

```bash
git pull --ff-only
```

如果本地有未提交修改、分支无法快进或发生冲突，Git 会正常失败，应用会展示 stdout/stderr，不会自动覆盖本地文件。

## 已知边界

- 下载过程当前由 GUI 等待后端任务完成后展示摘要；暂未做逐文件实时进度条。
- 选择粒度目前是文件和当前可见列表，后续可以扩展为目录级队列。
- 更新 R2 对象和生成 manifest 仍使用命令行工具，暂不放入 GUI。
