# rgt Installer Design

**Date:** 2026-02-21
**Goal:** Ship a real install experience for rgt — pre-built binaries on GitHub Releases, downloaded via a single `curl | sh` command. No Rust required to install.

---

## Architecture

Three components:

1. **GitHub Actions release workflow** — builds cross-platform binaries on tag push, creates GitHub Release, attaches tarballs
2. **`install.sh`** — detects OS/arch, downloads the right binary, verifies checksum, installs to `/usr/local/bin`
3. **`build.rs`** — bakes git tag + commit hash into binary at compile time for `rgt version`

---

## Component 1: Release Workflow

**File:** `.github/workflows/release.yml`

**Trigger:** `push` to tags matching `v*` (e.g. `v0.1.0`, `v0.2.0`)

**Build matrix:**

| Target triple | Runner | Archive name |
|---|---|---|
| `aarch64-apple-darwin` | `macos-latest` | `rgt-aarch64-apple-darwin.tar.gz` |
| `x86_64-apple-darwin` | `macos-latest` | `rgt-x86_64-apple-darwin.tar.gz` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `rgt-x86_64-unknown-linux-gnu.tar.gz` |

**Per-target steps:**
1. Checkout repo
2. Install Rust stable + add target triple
3. `cargo build --release --target <triple>`
4. Strip binary (reduces size ~60%)
5. Create `rgt-<target>.tar.gz` containing the `rgt` binary
6. Generate `rgt-<target>.tar.gz.sha256` checksum file
7. Upload both as artifacts

**Release step** (runs after all matrix jobs complete):
- Create GitHub Release using the tag name as the version
- Attach all tarballs and checksums
- Auto-generate release notes from commits since last tag

**To cut a release:**
```sh
git tag v0.2.0
git push --tags
```

---

## Component 2: install.sh

**File:** `install.sh` (repo root)

**User command:**
```sh
curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh
```

**With version pin:**
```sh
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/drewpayment/gastown-rusted/main/install.sh | sh
```

**Script logic:**

1. **Detect platform** — `uname` for OS, `uname -m` for arch
   - macOS arm64 → `aarch64-apple-darwin`
   - macOS x86_64 → `x86_64-apple-darwin`
   - Linux x86_64 → `x86_64-unknown-linux-gnu`
   - Anything else → error with message
2. **Resolve version** — use `$VERSION` env var if set, otherwise query GitHub API for latest release tag
3. **Download** tarball + `.sha256` file from GitHub Releases
4. **Verify checksum** — `sha256sum` (Linux) or `shasum -a 256` (macOS)
5. **Install binary** — extract `rgt` to `/usr/local/bin/rgt`, using `sudo` if needed
6. **Confirm** — run `rgt version` to verify, print next step (`rgt install`)

**Error handling:**
- Missing dependencies (`curl`, `tar`) → clear error message
- Unsupported platform → clear error message with manual build instructions
- Checksum mismatch → abort, delete partial download
- Install dir not writable → retry with sudo, fall back to `~/.local/bin` with PATH instructions

---

## Component 3: Version Embedding (build.rs)

**File:** `crates/gtr-cli/build.rs`

At build time, query `git describe --tags --always --dirty` and expose as `GIT_VERSION` env var for the binary. Falls back to `CARGO_PKG_VERSION` if not in a git repo (e.g. building from a tarball download).

**`rgt version` output:**
```
rgt 0.2.0 (abc1234)
Built: 2026-02-21
```

If built from source outside a git repo:
```
rgt 0.2.0 (source)
```

---

## Release Checklist (for maintainers)

```sh
# 1. Bump version in Cargo.toml workspace
#    [workspace.package] version = "0.2.0"

# 2. Update CHANGELOG or release notes (optional)

# 3. Commit and tag
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.2.0"
git tag v0.2.0
git push origin main --tags

# 4. GitHub Actions builds and publishes automatically
# 5. Verify at: https://github.com/drewpayment/gastown-rusted/releases
```

---

## What's Not In Scope

- **Homebrew tap** — can add later once there are several stable releases
- **`rgt update` command** — re-running install.sh is sufficient for now
- **Windows** — tmux dependency makes rgt Unix-only
- **ARM Linux** — can add `aarch64-unknown-linux-gnu` cross-compilation later if needed
- **crates.io publishing** — Temporal SDK pins to a git rev, making crates.io publish impractical for now
