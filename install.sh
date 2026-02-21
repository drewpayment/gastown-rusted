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
    ARCHIVE_PATH="$1"
    EXPECTED_CHECKSUM="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL="$(sha256sum "$ARCHIVE_PATH" | cut -d' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        ACTUAL="$(shasum -a 256 "$ARCHIVE_PATH" | cut -d' ' -f1)"
    else
        say "Warning: no sha256 tool found, skipping checksum verification."
        return
    fi

    if [ "$ACTUAL" != "$EXPECTED_CHECKSUM" ]; then
        err "Checksum verification failed. Expected: $EXPECTED_CHECKSUM  Got: $ACTUAL"
    fi
}

# ── install ───────────────────────────────────────────────────────────────────

install_binary() {
    BINARY_PATH="$1"

    # Create install dir if it doesn't exist
    if [ ! -d "$INSTALL_DIR" ]; then
        if mkdir -p "$INSTALL_DIR" 2>/dev/null; then
            : # created successfully
        else
            say "Creating $INSTALL_DIR requires sudo..."
            sudo mkdir -p "$INSTALL_DIR"
        fi
    fi

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
        API_RESPONSE="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>&1)" || \
            err "Failed to fetch release info from GitHub API. You may be rate-limited. Set VERSION env var to install a specific version, e.g.: VERSION=v0.1.0"
        VERSION="$(printf '%s' "$API_RESPONSE" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
        [ -n "$VERSION" ] || err "Could not parse release version from GitHub API response. Set VERSION env var manually."
    fi
    say "Version:  $VERSION"

    ARCHIVE="rgt-${TARGET}.tar.gz"
    BASE_URL="https://github.com/$REPO/releases/download/$VERSION"

    # Download to temp dir
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT

    say "Downloading $ARCHIVE..."
    curl -fsSL "$BASE_URL/$ARCHIVE" -o "$TMP/$ARCHIVE"

    say "Downloading checksum..."
    CHECKSUM_LINE="$(curl -fsSL "$BASE_URL/$ARCHIVE.sha256")"
    # Checksum file format: "<hash>  <filename>" — extract just the hash
    EXPECTED_CHECKSUM="$(printf '%s' "$CHECKSUM_LINE" | cut -d' ' -f1)"

    say "Verifying checksum..."
    verify_checksum "$TMP/$ARCHIVE" "$EXPECTED_CHECKSUM"

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
