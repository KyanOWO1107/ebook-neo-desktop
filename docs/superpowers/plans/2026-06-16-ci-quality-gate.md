# CI Quality Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a GitHub Actions quality gate for the independent desktop GUI repository.

**Architecture:** The workflow runs on Ubuntu and mirrors the local checks we already trust: npm dependency install, frontend tests, frontend build, npm audit, Rust formatting, Rust tests, and Rust compile check. It does not build signed installers or upload release artifacts; release packaging remains a later design topic.

**Tech Stack:** GitHub Actions, Node.js 22, npm, Rust stable, Cargo, Tauri v2 Linux build dependencies.

---

### Task 1: Add CI Workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Modify: `src-tauri/src/manifest.rs`
- Modify: `README.md`

- [x] **Step 1: Establish the local baseline**

Run:

```powershell
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
Set-Location src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

Expected: all commands exit 0.

- [x] **Step 2: Create the workflow**

Add `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches:
      - main
  pull_request:
    branches:
      - main

permissions:
  contents: read

jobs:
  quality:
    name: Test and build
    runs-on: ubuntu-22.04
    timeout-minutes: 30
    env:
      CARGO_BUILD_JOBS: "1"

    steps:
      - name: Checkout
        uses: actions/checkout@v6

      - name: Install Tauri Linux dependencies
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
            wget

      - name: Set up Node.js
        uses: actions/setup-node@v6
        with:
          node-version: "22"
          cache: npm

      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt

      - name: Cache Cargo registry and build output
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri -> target

      - name: Install npm dependencies
        run: npm ci

      - name: Run frontend tests
        run: npm test

      - name: Build frontend
        run: npm run build

      - name: Audit npm dependencies
        run: npm audit --audit-level=moderate --registry=https://registry.npmjs.org

      - name: Check Rust formatting
        working-directory: src-tauri
        run: cargo fmt --check

      - name: Run Rust tests
        working-directory: src-tauri
        run: cargo test

      - name: Check Rust build
        working-directory: src-tauri
        run: cargo check
```

- [x] **Step 3: Document the CI gate**

Before documenting, make the existing fake-rclone Rust tests platform-neutral so the Ubuntu runner can execute them without requiring PowerShell.

Update `README.md` so the development section lists the CI-equivalent local checks:

```bash
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
```

and:

```bash
cargo fmt --check
cargo test
cargo check
```

- [x] **Step 4: Verify locally**

Run:

```powershell
npm test
npm run build
npm audit --audit-level=moderate --registry=https://registry.npmjs.org
Set-Location src-tauri
cargo fmt --check
$env:CARGO_BUILD_JOBS='1'; cargo test
$env:CARGO_BUILD_JOBS='1'; cargo check
```

Expected: all commands exit 0.

- [x] **Step 5: Commit**

Run:

```powershell
git status --short
git add .github/workflows/ci.yml README.md docs/superpowers/plans/2026-06-16-ci-quality-gate.md
git commit -m "ci: add desktop quality gate"
```
