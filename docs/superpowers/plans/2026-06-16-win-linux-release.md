# Windows Linux Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a GitHub Actions release workflow that builds and uploads Ebook Neo Desktop bundles for Windows and Linux only.

**Architecture:** Keep the existing CI workflow as the quality gate for every push and pull request. Add a separate release workflow that runs on version tags or manual dispatch, uses `tauri-apps/tauri-action`, and uploads draft GitHub Release artifacts from `windows-latest` and `ubuntu-22.04`.

**Tech Stack:** GitHub Actions, Tauri v2, `tauri-apps/tauri-action@v0`, Node 22, Rust stable, Ubuntu 22.04, Windows latest.

---

### Task 1: Add Windows/Linux Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `README.md`

- [x] **Step 1: Add release workflow**

Create `.github/workflows/release.yml` with:

```yaml
name: Release Desktop

on:
  workflow_dispatch:
    inputs:
      version:
        description: "Release version, for example 0.1.0"
        required: true
        type: string
  push:
    tags:
      - "ebook-neo-desktop-v*"

permissions:
  contents: write

jobs:
  release:
    name: Build ${{ matrix.name }}
    runs-on: ${{ matrix.platform }}
    timeout-minutes: 60
    env:
      CARGO_BUILD_JOBS: "1"
      RELEASE_VERSION: ${{ github.event_name == 'workflow_dispatch' && inputs.version || github.ref_name }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: Windows
            platform: windows-latest
            args: "--bundles msi,nsis"
          - name: Linux
            platform: ubuntu-22.04
            args: "--bundles deb,appimage"

    steps:
      - name: Checkout
        uses: actions/checkout@v6

      - name: Install Tauri Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \
            build-essential \
            curl \
            file \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libssl-dev \
            libwebkit2gtk-4.1-dev \
            libxdo-dev \
            patchelf \
            wget

      - name: Set up Node.js
        uses: actions/setup-node@v6
        with:
          node-version: "22"
          cache: npm

      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo registry and build output
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri -> target

      - name: Install npm dependencies
        run: npm ci

      - name: Run frontend tests
        run: npm test

      - name: Audit npm dependencies
        run: npm audit --audit-level=moderate --registry=https://registry.npmjs.org

      - name: Run Rust tests
        working-directory: src-tauri
        run: cargo test

      - name: Build and upload bundles
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          args: ${{ matrix.args }}
          tagName: ${{ github.event_name == 'workflow_dispatch' && format('ebook-neo-desktop-v{0}', inputs.version) || github.ref_name }}
          releaseName: "Ebook Neo Desktop ${{ env.RELEASE_VERSION }}"
          releaseBody: |
            Windows and Linux desktop builds for Ebook Neo.

            Notes:
            - macOS builds are intentionally not included in this workflow.
            - rclone and the read-only R2 remote are still external collaborator prerequisites.
            - These builds are unsigned; Windows users may see a SmartScreen warning.
          releaseDraft: true
          prerelease: false
```

- [x] **Step 2: Update README release notes**

Add a `发布流程` section documenting:

- Windows/Linux releases are produced by `.github/workflows/release.yml`.
- The workflow can be run manually or by pushing a tag like `ebook-neo-desktop-v0.1.0`.
- The generated GitHub Release is a draft.
- macOS is intentionally excluded for now.
- Windows builds are unsigned and may trigger SmartScreen.
- rclone remains an external prerequisite.

- [x] **Step 3: Verify**

Run:

```powershell
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
Set-Location src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
Set-Location ..
git diff --check
```

- [x] **Step 4: Commit**

```powershell
git add .github/workflows/release.yml README.md docs/superpowers/plans/2026-06-16-win-linux-release.md
git commit -m "ci: add windows linux release workflow"
```
