#!/usr/bin/env bash
# @anchor infra:distribution:install
# @tags infra
#
# install.sh — one-line installer for the testudo trading harness.
#
# Usage:
#   curl -fsSL https://testudo.vip/install.sh | bash
#
# Detects OS/arch, downloads the latest binary from GitHub Releases,
# installs to ~/.local/bin, and appends to the user's shell PATH.
# Idempotent — safe to re-run to update the binary.

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────

readonly REPO="sub0xdai/testudo"
readonly BINARY="testudo"
readonly INSTALL_DIR="${HOME}/.local/bin"
readonly RELEASE_URL="https://github.com/${REPO}/releases/latest/download"

# ── Preconditions ──────────────────────────────────────────────

command -v curl >/dev/null 2>&1 || {
    echo "Error: curl is required to download testudo. Install curl and try again."
    exit 1
}

command -v tar >/dev/null 2>&1 || {
    echo "Error: tar is required to extract the testudo binary."
    exit 1
}

# ── OS / Arch detection ────────────────────────────────────────

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        echo "Error: Unsupported architecture: $(uname -m)"
        echo "Supported: x86_64 (amd64), aarch64 (arm64)"
        exit 1
        ;;
esac

case "$OS" in
    linux)
        TARGET="${ARCH}-unknown-linux-gnu"
        ;;
    darwin)
        if [ "$ARCH" = "aarch64" ]; then
            TARGET="aarch64-apple-darwin"
        else
            TARGET="x86_64-apple-darwin"
        fi
        ;;
    *)
        echo "Error: Unsupported OS: $OS"
        echo "Supported: linux, darwin (macOS)"
        exit 1
        ;;
esac

# ── Download ───────────────────────────────────────────────────

TARBALL="${BINARY}-${TARGET}.tar.gz"
DOWNLOAD_URL="${RELEASE_URL}/${TARBALL}"
TMP_TARBALL="/tmp/${TARBALL}"

echo "→ Downloading testudo for ${OS}/${ARCH}..."
echo "  ${DOWNLOAD_URL}"

http_code=$(curl -fsSL -w '%{http_code}' -o "$TMP_TARBALL" "$DOWNLOAD_URL" 2>/dev/null) || {
    echo ""
    echo "Error: Failed to download testudo."
    echo "The release may not exist yet for ${TARGET}."
    echo "Check https://github.com/${REPO}/releases for available builds."
    rm -f "$TMP_TARBALL"
    exit 1
}

if [ "$http_code" != "200" ]; then
    echo ""
    echo "Error: GitHub returned HTTP ${http_code}."
    echo "The release may not exist yet for ${TARGET}."
    rm -f "$TMP_TARBALL"
    exit 1
fi

# ── Verify tarball is not empty / not HTML error page ──────────

file_type=$(file -b --mime-type "$TMP_TARBALL" 2>/dev/null || echo "unknown")
case "$file_type" in
    application/gzip|application/x-gzip|application/x-tar|application/octet-stream)
        ;;
    *)
        echo "Error: Downloaded file is not a valid tarball (type: ${file_type})."
        rm -f "$TMP_TARBALL"
        exit 1
        ;;
esac

# ── Extract ────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"

if ! tar -xzf "$TMP_TARBALL" -C "$INSTALL_DIR" 2>/dev/null; then
    echo "Error: Failed to extract tarball."
    echo "The file may be corrupted or the release format has changed."
    rm -f "$TMP_TARBALL"
    exit 1
fi

chmod +x "${INSTALL_DIR}/${BINARY}" 2>/dev/null || {
    echo "Error: Failed to set executable permission on ${INSTALL_DIR}/${BINARY}"
    rm -f "$TMP_TARBALL"
    exit 1
}

rm -f "$TMP_TARBALL"

# ── Postcondition: binary exists and is executable ─────────────

if [ ! -x "${INSTALL_DIR}/${BINARY}" ]; then
    echo "Error: Binary not found or not executable after install: ${INSTALL_DIR}/${BINARY}"
    exit 1
fi

# ── Version check ──────────────────────────────────────────────

installed_version=$("${INSTALL_DIR}/${BINARY}" --version 2>/dev/null || echo "unknown")
echo "   Installed: ${installed_version}"

# ── Add to PATH ────────────────────────────────────────────────

add_to_path() {
    local rc_file="$1"
    local export_line="export PATH=\"${INSTALL_DIR}:\$PATH\""

    if [ -f "$rc_file" ]; then
        if ! grep -qF "${INSTALL_DIR}" "$rc_file" 2>/dev/null; then
            # Ensure a trailing newline before appending
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
        # File doesn't exist — create it
        echo "# testudo" > "$rc_file"
        echo "$export_line" >> "$rc_file"
        echo "   Created ${rc_file} with PATH entry"
    fi
}

SHELL_NAME=$(basename "${SHELL:-/bin/bash}")

case "$SHELL_NAME" in
    zsh)
        add_to_path "${HOME}/.zshrc"
        ;;
    bash)
        # .bashrc: some distros use .bash_profile instead; handle both
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
    *)
        add_to_path "${HOME}/.profile"
        ;;
esac

# ── Done ───────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════════════════════"
echo "  ✅ testudo installed successfully!"
echo "     Binary: ${INSTALL_DIR}/${BINARY}"
echo "══════════════════════════════════════════════════════════════"
echo ""

case "$SHELL_NAME" in
    fish)
        echo "  Restart your shell or run:"
        echo "    exec fish"
        ;;
    *)
        echo "  Restart your shell or run:"
        echo "    source ~/.${SHELL_NAME}rc"
        ;;
esac

echo ""
echo "  Next steps:"
echo "    testudo init        Complete setup wizard"
echo "    testudo --help      See all commands"
echo ""
