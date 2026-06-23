# Ebook Neo Desktop 发布检查清单

这份清单面向维护者，用于发布 Windows 和 Linux 协作者稳定版。当前发布不包含 macOS、不包含代码签名、不内置 rclone 或 R2 凭据。

## 发布定位

协作者稳定版应满足：

- 安装包来自 GitHub Release draft。
- Windows 和 Linux 产物都由 GitHub Actions 构建。
- 使用文档能覆盖安装、rclone 配置、索引仓库设置、更新资源表、下载和常见故障。
- 维护者已经在至少一台 Windows 机器上手动完成启动、检查 R2、更新资源表和下载小文件测试。
- Release 说明明确当前安装包未签名，Windows 可能出现 SmartScreen 提示。

## 1. 发布前本地检查

确认工作区干净，或只有本次发布所需改动：

```bash
git status --short --branch
```

运行质量门：

```bash
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
```

Rust 后端：

```bash
cd src-tauri
cargo fmt --check
cargo test
cargo check
```

Windows 上可以限制 Cargo 并发：

```powershell
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

## 2. 版本号检查

确认这些文件中的版本一致：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

如果版本只存在于 draft release、尚未正式给协作者发布，可以删除 draft 和远程 tag 后重打同一个版本。若版本已经公开发布或已被协作者下载使用，修复安装器或升级问题时应发布新的 patch 版本，例如从 `1.0.1` 升到 `1.0.2`，不要复用已公开 tag。

当前发布 tag 格式：

```text
ebook-neo-desktop-v<version>
```

例如：

```text
ebook-neo-desktop-v0.1.0
```

## 3. 推送代码

确认本地提交已包含本次发布文档和功能：

```bash
git log --oneline -5
git status --short --branch
```

由维护者手动推送：

```bash
git push origin main
```

等待 CI 通过后再触发发布。

## 4. 触发 Release workflow

推荐先手动触发：

1. 打开 GitHub 仓库的 Actions 页面。
2. 选择 `Release Desktop`。
3. 点击 `Run workflow`。
4. 填写版本号，例如 `0.1.0`。

也可以用 tag 触发：

```bash
git tag ebook-neo-desktop-v0.1.0
git push origin ebook-neo-desktop-v0.1.0
```

workflow 会创建 draft release，不会立即公开发布。

## 5. 检查 draft release

Windows 应包含：

- `.msi`
- NSIS `.exe`

Linux 应包含：

- `.deb`
- `.AppImage`

Release body 至少应说明：

- macOS 暂不包含。
- rclone 和只读 R2 remote 仍需协作者自行配置。
- 当前构建未签名，Windows 可能出现 SmartScreen 提示。
- 推荐协作者先阅读 `docs/collaborator-install.md`。

## 6. 发布前手动 smoke test

至少在 Windows 上测试：

1. 安装 `.exe` 或 `.msi`。
2. 启动 Ebook Neo Desktop。
3. 设置索引仓库路径。
4. 设置 rclone 路径、Remote 和 Bucket。
5. 点击“检查 R2”。
6. 点击“更新资源表”。
7. 下载一个小文件。
8. 下载一个大于“大文件阈值”的文件，确认进度更新。
9. 点击“打开目录”，确认目标目录可打开。

从 `1.0.0` 升级到 `1.0.1` 及之后版本时，还应额外测试 NSIS `.exe` 的覆盖安装路径。`1.0.0` 使用旧 Publisher `tyutebooks`，`1.0.1` 起使用 `Kyanetwork`；安装器模板需要保留旧厂商注册表键的兼容逻辑，才能在选择“安装前卸载”时正确启动旧卸载器。

Linux 测试如果有机器可用，优先测试 `.AppImage`。没有 Linux 机器时，至少确认 GitHub Actions 的 Linux job 通过，并在 Release 说明中保留反馈入口。

## 7. 发布给协作者

发布 draft release 后，给协作者的信息应包含：

- Release 链接。
- 需要先配置 rclone 只读 remote。
- 需要本地克隆 `TYUT-ebooks-collection-neo`。
- Windows 未签名提示是当前已知情况。
- 遇到问题时提供平台、rclone 版本、文件路径和 GUI 报错信息。

## 回滚

如果发布后发现严重问题：

1. 在 GitHub Release 页面将该版本标记为 pre-release 或撤回发布。
2. 在协作者通知中说明暂停使用该版本。
3. 修复后发布新 patch 版本，例如 `0.1.1`。
4. 不要复用已经公开发布过的 tag。
