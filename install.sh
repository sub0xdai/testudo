#!/usr/bin/env bash
# @anchor infra:distribution:install
# @tags infra
#
# install.sh — one-line installer for the testudo trading harness.
#
# Usage:
#   curl -fsSL https://api.testudo.vip/install.sh | bash
#
# Supports Linux, macOS, and Windows (Git Bash / WSL / MSYS2).
# Detects OS/arch, downloads the latest binary from GitHub Releases,
# installs to ~/.local/bin (Unix) or ~/bin (Windows), and appends to PATH.
# Idempotent — safe to re-run to update the binary.

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────

readonly REPO="sub0xdai/testudo"
readonly BINARY="testudo"
readonly RELEASE_URL="https://github.com/${REPO}/releases/latest/download"

# ── Preconditions ──────────────────────────────────────────────

command -v curl >/dev/null 2>&1 || {
    echo "Error: curl is required. Install curl and try again."
    exit 1
}

# ── OS / Arch detection ────────────────────────────────────────

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)
        echo "Error: Unsupported architecture: $(uname -m)"
        echo "Supported: x86_64 (amd64), aarch64 (arm64)"
        exit 1
        ;;
esac

IS_WINDOWS=false
IS_UNIX=false

case "$OS" in
    linux)
        TARGET="${ARCH}-unknown-linux-gnu"
        INSTALL_DIR="${HOME}/.local/bin"
        EXT="tar.gz"
        IS_UNIX=true
        ;;
    darwin)
        TARGET="${ARCH}-apple-darwin"
        INSTALL_DIR="${HOME}/.local/bin"
        EXT="tar.gz"
        IS_UNIX=true
        ;;
    mingw*|msys*|cygwin*|windowsnt)
        TARGET="${ARCH}-pc-windows-msvc"
        INSTALL_DIR="${HOME}/bin"
        EXT="zip"
        IS_WINDOWS=true
        ;;
    *)
        echo "Error: Unsupported OS: $OS"
        echo "Supported: linux, darwin (macOS), mingw/msys (Windows)"
        exit 1
        ;;
esac

# ── Download ───────────────────────────────────────────────────

ARCHIVE="${BINARY}-${TARGET}.${EXT}"
DOWNLOAD_URL="${RELEASE_URL}/${ARCHIVE}"
TMP_ARCHIVE="/tmp/${ARCHIVE}"

echo "→ Downloading testudo for ${OS}/${ARCH}..."
echo "  ${DOWNLOAD_URL}"

http_code=$(curl -fsSL -w '%{http_code}' -o "$TMP_ARCHIVE" "$DOWNLOAD_URL" 2>/dev/null) || {
    echo ""
    echo "Error: Failed to download testudo."
    echo "The release may not exist yet for ${TARGET}."
    echo "Check https://github.com/${REPO}/releases for available builds."
    rm -f "$TMP_ARCHIVE"
    exit 1
}

if [ "$http_code" != "200" ]; then
    echo ""
    echo "Error: GitHub returned HTTP ${http_code}."
    echo "The release may not exist yet for ${TARGET}."
    rm -f "$TMP_ARCHIVE"
    exit 1
fi

# ── Extract ────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"

if $IS_WINDOWS; then
    # Windows: unzip
    command -v unzip >/dev/null 2>&1 || {
        echo "Error: unzip is required on Windows. Install it and try again."
        rm -f "$TMP_ARCHIVE"
        exit 1
    }
    if ! unzip -o "$TMP_ARCHIVE" -d "$INSTALL_DIR" 2>/dev/null; then
        echo "Error: Failed to extract zip."
        rm -f "$TMP_ARCHIVE"
        exit 1
    fi
else
    # Unix: tar.gz
    command -v tar >/dev/null 2>&1 || {
        echo "Error: tar is required. Install it and try again."
        rm -f "$TMP_ARCHIVE"
        exit 1
    }
    if ! tar -xzf "$TMP_ARCHIVE" -C "$INSTALL_DIR" 2>/dev/null; then
        echo "Error: Failed to extract tarball."
        rm -f "$TMP_ARCHIVE"
        exit 1
    fi
    chmod +x "${INSTALL_DIR}/${BINARY}" 2>/dev/null || {
        echo "Error: Failed to set executable permission."
        rm -f "$TMP_ARCHIVE"
        exit 1
    }
fi

rm -f "$TMP_ARCHIVE"

# ── Postcondition: binary exists ───────────────────────────────

BIN_PATH="${INSTALL_DIR}/${BINARY}"
if $IS_WINDOWS; then
    BIN_PATH="${INSTALL_DIR}/${BINARY}.exe"
fi

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found after install: ${BIN_PATH}"
    exit 1
fi

# ── Version check ──────────────────────────────────────────────

installed_version=$("$BIN_PATH" --version 2>/dev/null || echo "unknown")
echo "   Installed: ${installed_version}"

# ── Add to PATH ────────────────────────────────────────────────

if $IS_UNIX; then
    add_to_path() {
        local rc_file="$1"
        local export_line="export PATH=\"${INSTALL_DIR}:\$PATH\""

        if [ -f "$rc_file" ]; then
            if ! grep -qF "${INSTALL_DIR}" "$rc_file" 2>/dev/null; then
                if [ -s "$rc_file" ] && [ "$(tail -c 1 "$rc_file")" != "" ]; then
                    echo "" >> "$rc_file"
                fi
                echo "# testudo" >> "$rc_file"
                echo "$export_line" >> "$rc_file"
                echo "   Added to ${rc_file}"
            else
                echo "   PATH already configured in ${rc_file}"
            fi
        else
            echo "# testudo" > "$rc_file"
            echo "$export_line" >> "$rc_file"
            echo "   Created ${rc_file} with PATH entry"
        fi
    }

    SHELL_NAME=$(basename "${SHELL:-/bin/bash}")

    case "$SHELL_NAME" in
        zsh)  add_to_path "${HOME}/.zshrc" ;;
        bash)
            if [ -f "${HOME}/.bash_profile" ]; then
                add_to_path "${HOME}/.bash_profile"
            else
                add_to_path "${HOME}/.bashrc"
            fi
            ;;
        fish)
            fish_config="${HOME}/.config/fish/config.fish"
            mkdir -p "$(dirname "$fish_config")"
            if [ -f "$fish_config" ]; then
                if ! grep -qF "${INSTALL_DIR}" "$fish_config" 2>/dev/null; then
                    if [ -s "$fish_config" ] && [ "$(tail -c 1 "$fish_config")" != "" ]; then
                        echo "" >> "$fish_config"
                    fi
                    echo "# testudo" >> "$fish_config"
                    echo "fish_add_path ${INSTALL_DIR}" >> "$fish_config"
                    echo "   Added to ${fish_config}"
                else
                    echo "   PATH already configured in ${fish_config}"
                fi
            else
                echo "# testudo" > "$fish_config"
                echo "fish_add_path ${INSTALL_DIR}" >> "$fish_config"
                echo "   Created ${fish_config} with PATH entry"
            fi
            ;;
        *) add_to_path "${HOME}/.profile" ;;
    esac
else
    # Windows: add to user PATH via setx
    echo "   On Windows, add ${INSTALL_DIR} to your PATH:"
    echo "   setx PATH \"%PATH%;${INSTALL_DIR}\""
fi

# ── Done ───────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════════════════════"
echo "  ✅ testudo installed successfully!"
echo "     Binary: ${BIN_PATH}"
echo "══════════════════════════════════════════════════════════════"
echo ""

if $IS_UNIX; then
    case "${SHELL_NAME:-}" in
        fish) echo "  Restart your shell or run: exec fish" ;;
        *)    echo "  Restart your shell or run: source ~/.${SHELL_NAME:-bash}rc" ;;
    esac
fi

echo ""
echo "  Next steps:"
echo "    testudo init        Complete setup wizard"
echo "    testudo --help      See all commands"
echo ""
