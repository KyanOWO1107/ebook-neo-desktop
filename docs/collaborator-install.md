# Ebook Neo Desktop 协作者安装指南

这份指南面向只需要下载资料的协作者。维护者会通过 GitHub Release 提供 Windows 和 Linux 安装包；你不需要安装 Node.js、npm、Rust 或 Cargo。

## 需要准备

- Git
- rclone
- `TYUT-ebooks-collection-neo` 索引仓库本地克隆
- 维护者提供的 Cloudflare R2 只读凭据
- Ebook Neo Desktop 安装包

资料文件本体存放在私有 Cloudflare R2 Bucket 中。只有索引仓库或 GUI 安装包不能直接下载资料，必须配置只读 R2 remote。

## 1. 安装 rclone

从 rclone 官网下载并安装：

```text
https://rclone.org/downloads/
```

Windows 上建议把 `rclone.exe` 所在目录加入 PATH。没有加入 PATH 也可以，后续在 GUI 设置页填写 `rclone.exe` 的绝对路径。

Linux 上通常可以通过发行版包管理器或 rclone 官方安装方式安装。确认命令：

```bash
rclone version
```

## 2. 配置只读 R2 remote

向维护者索取：

- Account ID
- Access Key ID
- Secret Access Key
- Bucket 名称：`tyut-ebooks-collection-neo`

运行：

```bash
rclone config
```

推荐 remote 名称：

```text
ebookneo-r2-readonly
```

配置要点：

```text
Storage: s3
Provider: Cloudflare
Access key ID: 维护者提供
Secret access key: 维护者提供
Endpoint: https://<Account ID>.r2.cloudflarestorage.com
ACL: private 或留空
```

配置后测试：

```bash
rclone lsf ebookneo-r2-readonly:tyut-ebooks-collection-neo --max-depth 1
```

能看到 `objects/` 或命令成功返回，说明 remote 可用。

## 3. 克隆索引仓库

```bash
git clone <TYUT-ebooks-collection-neo 的 GitHub 地址>
cd TYUT-ebooks-collection-neo
git pull --ff-only
```

确认存在：

```text
manifests/files.jsonl
```

推荐目录结构：

```text
Ebook/
  TYUT-ebooks-collection-neo/
```

GUI 可以安装在任意位置，但首次启动后需要在“设置”页指向这个索引仓库目录。

## 4. 安装 Ebook Neo Desktop

从维护者发布的 GitHub Release 下载对应平台安装包。

Windows:

- 优先使用 NSIS `.exe` 安装包。
- `.msi` 也可使用，适合需要 Windows Installer 的环境。
- 当前安装包未签名，Windows 可能出现 SmartScreen 提示。确认来源是维护者发布的 GitHub Release 后再继续安装。

Linux:

- `.AppImage` 适合大多数桌面发行版。首次运行前可能需要：

```bash
chmod a+x Ebook*.AppImage
```

- `.deb` 适合 Debian、Ubuntu 及其衍生发行版。

macOS:

- 当前稳定版发布暂不提供 macOS 安装包。macOS 协作者可暂时从源码运行，或等待后续 macOS 发布方案。

## 5. 首次设置

打开 Ebook Neo Desktop，进入“设置”页：

- `索引仓库`：选择本地 `TYUT-ebooks-collection-neo` 目录。
- `下载目录`：选择你希望保存资料的位置。
- `rclone`：填写 `rclone`，或填写 rclone 可执行文件绝对路径。
- `Remote`：填写 `ebookneo-r2-readonly`。
- `Bucket`：填写 `tyut-ebooks-collection-neo`。
- `并发`：默认 4。网络不稳定时可以调低到 1 或 2。
- `大文件阈值`：默认 20 MiB。
- `大文件线程`：默认 8。
- `大文件下载进度展示`：建议保持开启。

点击“保存设置”，再点击“检查 R2”。检查通过后即可下载。

## 6. 更新和下载资料

常用流程：

1. 点击“更新资源表”，从 GitHub 拉取最新 `manifests/files.jsonl`。
2. 在“资料”页搜索或展开左侧目录。
3. 勾选需要下载的文件或目录。
4. 点击“开始下载”。
5. 在“下载”页查看逐文件进度、失败项和重试入口。

下载完成后，GUI 会校验文件大小和 sha256。校验通过后才会把 `.ebook-neo-part` 临时文件移动成最终文件名。

## 常见问题

### 检查 R2 失败

优先检查：

- Remote 名称是否和 GUI 设置一致。
- Endpoint 是否为 `https://<Account ID>.r2.cloudflarestorage.com`。
- Access Key 和 Secret 是否来自只读 token。
- Bucket 是否为 `tyut-ebooks-collection-neo`。

可在终端运行：

```bash
rclone lsf ebookneo-r2-readonly:tyut-ebooks-collection-neo --max-depth 1
```

### 更新资源表失败

GUI 内部执行：

```bash
git pull --ff-only
```

普通协作者不要在索引仓库中直接修改文件。如果已经修改，可以先备份自己的改动，再恢复索引仓库后重新更新。

### 下载速度慢

可以尝试：

- 关闭或切换代理。
- 在“设置”页降低文件级并发，避免多个大文件抢带宽。
- 对单个大文件保留 `大文件线程` 为 8，必要时尝试 4 或 12。

### 大文件没有显示进度

确认“设置”页里的 `大文件下载进度展示` 已开启。该功能依赖 rclone 的 `--progress` 输出；如果下载仍可完成但没有中间进度，请记录文件路径、平台、rclone 版本并反馈给维护者。
