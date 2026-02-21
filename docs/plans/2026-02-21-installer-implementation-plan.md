# rgt Installer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship a `curl | sh` installer for rgt backed by GitHub Actions pre-built binaries and version-embedded builds.

**Architecture:** Three components — a `build.rs` that bakes git version info into the binary, a GitHub Actions workflow that builds and publishes release tarballs on tag push, and an `install.sh` that detects platform, downloads the right binary, verifies checksum, and installs to `/usr/local/bin`.

**Tech Stack:** Rust `build.rs`, GitHub Actions matrix build, POSIX sh, `cargo build --release`, `strip`, `sha256sum`/`shasum`

**Codebase root:** `/Users/drew.payment/dev/gastown-rusted/gtr/crew/drew`

---

## Task 1: Add `build.rs` — Embed Git Version at Compile Time

**Files:**
- Create: `crates/gtr-cli/build.rs`
- Modify: `crates/gtr-cli/src/commands/mod.rs` (no change needed — version is read via `env!` macro in main.rs)
- Modify: `crates/gtr-cli/src/main.rs` (update Version command)

**Step 1: Create `build.rs`**

```rust
// crates/gtr-cli/build.rs
fn main() {
    // Embed git describe output (e.g. "v0.1.0-3-gabc1234" or "abc1234")
    let git_version = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "source".to_string());

    println!("cargo:rustc-env=GIT_VERSION={git_version}");

    // Embed build date
    let build_date = std::process::Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    // Re-run if HEAD changes
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
}
```

**Step 2: Update the Version command in `main.rs`**

Find the `Command::Version` match arm (currently around line 227) and replace it:

```rust
Command::Version => {
    println!(
        "rgt {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_VERSION")
    );
    println!("Built: {}", env!("BUILD_DATE"));
    Ok(())
}
```

**Step 3: Build and verify**

Run: `cargo build -p gtr-cli 2>&1 | tail -5`
Expected: Compiles without error.

Run: `./target/debug/rgt version`
Expected output (example):
```
rgt 0.1.0 (abc1234)
Built: 2026-02-21
```

**Step 4: Commit**

```bash
git add crates/gtr-cli/build.rs crates/gtr-cli/src/main.rs
git commit -m "feat: embed git version and build date via build.rs"
```

---

## Task 2: Create GitHub Actions Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Create the workflow directory**

```bash
mkdir -p .github/workflows
```

**Step 2: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0  # needed for git describe in build.rs

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-${{ matrix.target }}-cargo-

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }} -p gtr-cli

      - name: Strip binary (reduce size)
        run: strip target/${{ matrix.target }}/release/rgt

      - name: Package tarball
        run: |
          ARCHIVE=rgt-${{ matrix.target }}.tar.gz
          tar -czf "$ARCHIVE" -C target/${{ matrix.target }}/release rgt
          echo "ARCHIVE=$ARCHIVE" >> $GITHUB_ENV

      - name: Generate checksum
        run: |
          if command -v sha256sum >/dev/null 2>&1; then
            sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
          else
            shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"
          fi

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: rgt-${{ matrix.target }}
          path: |
            rgt-${{ matrix.target }}.tar.gz
            rgt-${{ matrix.target }}.tar.gz.sha256

  release:
    name: Create GitHub Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - name: Create release
        uses: softprops/action-gh-release@v2
        with:
          files: artifacts/**
          generate_release_notes: true
          fail_on_unmatched_files: true
```

**Step 3: Verify YAML syntax locally (optional)**

If you have `actionlint` installed:
```bash
actionlint .github/workflows/release.yml
```
Otherwise skip — GitHub will catch syntax errors on push.

**Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: GitHub Actions release workflow — build matrix for macOS arm64/x86 and Linux"
```

---

## Task 3: Create `install.sh`

**Files:**
- Create: `install.sh` (repo root)

**Step 1: Create `install.sh`**

```sh
#!/usr/bin/env sh
# install.sh — rgt installer
# Usage: curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh
# Or with a specific version:
#   VERSION=v0.1.0 curl -fsSL ... | sh

set -e

REPO="drewpayment/gastown-rusted"
INSTALL_DIR="/usr/local/bin"
BINARY="rgt"

# ── helpers ───────────────────────────────────────────────────────────────────

say() { printf "  %s\n" "$*"; }
err() { printf "\nerror: %s\n" "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || err "Required tool not found: $1. Please install it and try again."; }

# ── platform detection ────────────────────────────────────────────────────────

detect_target() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Darwin)
            case "$ARCH" in
                arm64)  echo "aarch64-apple-darwin" ;;
                x86_64) echo "x86_64-apple-darwin" ;;
                *)      err "Unsupported macOS architecture: $ARCH" ;;
            esac
            ;;
        Linux)
            case "$ARCH" in
                x86_64) echo "x86_64-unknown-linux-gnu" ;;
                *)      err "Unsupported Linux architecture: $ARCH. Build from source: https://github.com/$REPO" ;;
            esac
            ;;
        *)
            err "Unsupported OS: $OS. rgt requires macOS or Linux."
            ;;
    esac
}

# ── checksum verification ─────────────────────────────────────────────────────

verify_checksum() {
    ARCHIVE="$1"
    CHECKSUM_FILE="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum --check --status "$CHECKSUM_FILE" || err "Checksum verification failed. The download may be corrupted."
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 --check --status "$CHECKSUM_FILE" || err "Checksum verification failed. The download may be corrupted."
    else
        say "Warning: no sha256 tool found, skipping checksum verification."
    fi
}

# ── install ───────────────────────────────────────────────────────────────────

install_binary() {
    BINARY_PATH="$1"

    if [ -w "$INSTALL_DIR" ]; then
        cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY"
        chmod +x "$INSTALL_DIR/$BINARY"
    else
        say "Installing to $INSTALL_DIR requires sudo..."
        sudo cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY"
        sudo chmod +x "$INSTALL_DIR/$BINARY"
    fi
}

# ── main ──────────────────────────────────────────────────────────────────────

main() {
    printf "\nInstalling rgt...\n\n"

    need curl
    need tar

    TARGET="$(detect_target)"
    say "Platform: $TARGET"

    # Resolve version
    if [ -z "$VERSION" ]; then
        say "Fetching latest release..."
        VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep '"tag_name"' \
            | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
        [ -n "$VERSION" ] || err "Could not determine latest version. Set VERSION env var manually."
    fi
    say "Version:  $VERSION"

    ARCHIVE="rgt-${TARGET}.tar.gz"
    BASE_URL="https://github.com/$REPO/releases/download/$VERSION"

    # Download to temp dir
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT

    say "Downloading $ARCHIVE..."
    curl -fsSL "$BASE_URL/$ARCHIVE" -o "$TMP/$ARCHIVE"
    curl -fsSL "$BASE_URL/$ARCHIVE.sha256" -o "$TMP/$ARCHIVE.sha256"

    # Rewrite checksum file path to match tmp location
    sed -i.bak "s|rgt-${TARGET}.tar.gz|$TMP/$ARCHIVE|g" "$TMP/$ARCHIVE.sha256" 2>/dev/null || \
        sed -i "" "s|rgt-${TARGET}.tar.gz|$TMP/$ARCHIVE|g" "$TMP/$ARCHIVE.sha256"

    say "Verifying checksum..."
    verify_checksum "$TMP/$ARCHIVE" "$TMP/$ARCHIVE.sha256"

    say "Extracting..."
    tar -xzf "$TMP/$ARCHIVE" -C "$TMP"

    say "Installing to $INSTALL_DIR/$BINARY..."
    install_binary "$TMP/$BINARY"

    printf "\n"
    say "rgt installed successfully!"
    printf "\n"

    # Verify
    if command -v rgt >/dev/null 2>&1; then
        rgt version
    else
        say "Binary installed to $INSTALL_DIR/$BINARY"
        say "Make sure $INSTALL_DIR is in your PATH."
    fi

    printf "\nNext steps:\n"
    printf "  rgt install    # set up ~/.gtr directories\n"
    printf "  rgt start      # start Gas Town\n\n"
}

main "$@"
```

**Step 2: Make executable**

```bash
chmod +x install.sh
```

**Step 3: Smoke test locally (dry run — without pushing a real release)**

Verify the platform detection logic works:
```bash
sh -c 'OS=$(uname -s); ARCH=$(uname -m); echo "OS=$OS ARCH=$ARCH"'
```
Expected on Apple Silicon Mac: `OS=Darwin ARCH=arm64`

Verify the script is valid POSIX sh (no bashisms):
```bash
sh -n install.sh && echo "syntax ok"
```
Expected: `syntax ok`

**Step 4: Commit**

```bash
git add install.sh
git commit -m "feat: install.sh — curl-pipe installer with platform detection and checksum verification"
```

---

## Task 4: Update README with Install Instructions

**Files:**
- Modify: `README.md`

**Step 1: Replace the Installation section**

Find the existing `## Installation` section in `README.md` and replace it with:

```markdown
## Installation

### Quick install (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh
```

Installs the latest pre-built binary to `/usr/local/bin/rgt`. Supports macOS (arm64, x86_64) and Linux (x86_64).

To install a specific version:
```sh
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh
```

### Build from source

Requires [Rust stable](https://rustup.rs).

```sh
git clone git@github.com:drewpayment/gastown-rusted.git
cd gastown-rusted
cargo build --release -p gtr-cli
# Binary at: target/release/rgt
cp target/release/rgt /usr/local/bin/rgt
```

### First-time setup

After installing, run:
```sh
rgt install
```

This creates `~/.gtr/` directories and validates dependencies (tmux, temporal, claude).
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README install instructions — curl pipe installer"
```

---

## Task 5: Bump Version and Cut First Real Release

**Files:**
- Modify: `Cargo.toml` (workspace version)

**Step 1: Bump version in workspace Cargo.toml**

Find:
```toml
[workspace.package]
version = "0.1.0"
```

Change to:
```toml
[workspace.package]
version = "0.1.0"
```

(Keep at `0.1.0` for the first tagged release — this is our v0.1.0.)

**Step 2: Verify build is clean**

```bash
cargo build --release -p gtr-cli 2>&1 | tail -3
```
Expected: `Compiling gtr-cli v0.1.0 ...` then `Finished`.

**Step 3: Push everything and tag**

```bash
git push origin main
git tag v0.1.0
git push --tags
```

**Step 4: Watch the CI run**

Go to: `https://github.com/drewpayment/gastown-rusted/actions`

Expected: Three build jobs (aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu) all green, then the release job creates a GitHub Release at `https://github.com/drewpayment/gastown-rusted/releases/tag/v0.1.0`.

**Step 5: Test the installer end-to-end**

```bash
# Remove existing binary temporarily
which rgt  # note the path

# Run the installer
curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh

# Verify
rgt version
```

Expected:
```
rgt 0.1.0 (v0.1.0)
Built: 2026-02-21
```

---

## Notes

- **`fetch-depth: 0`** in the workflow is required so `git describe` in `build.rs` can find tags.
- **`softprops/action-gh-release@v2`** creates the release and attaches files in one step — no need for separate create/upload actions.
- **`sed -i.bak`** in `install.sh` handles the BSD sed difference between macOS and Linux (macOS requires a backup suffix argument, Linux doesn't).
- **Strip** reduces the binary from ~25MB to ~8MB on macOS.
- The `cargo:rerun-if-changed` directives in `build.rs` ensure version is re-embedded on every tag but Cargo doesn't rebuild unnecessarily on unrelated changes.
