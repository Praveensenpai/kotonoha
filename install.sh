#!/usr/bin/env bash
set -e

REPO="Praveensenpai/kotonoha"
BINARY_NAME="kotonoha"
INSTALL_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/kotonoha"

echo "🌸 Installing kotonoha (言の葉)..."

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ]; then
    echo "❌ Currently only Linux is supported by this installer."
    exit 1
fi

if [ "$ARCH" != "x86_64" ]; then
    echo "❌ Currently only x86_64 architecture is supported."
    exit 1
fi

# Deploy default config.toml if missing
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    echo "⚙️  Deploying default config to $CONFIG_DIR/config.toml..."
    curl -sSL "https://raw.githubusercontent.com/$REPO/main/config.toml.example" -o "$CONFIG_DIR/config.toml" || true
fi

TAG=$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$TAG" ]; then
    echo "⚠️  No official release tag found. Building from source via Cargo..."
    if command -v cargo >/dev/null 2>&1; then
        cargo install --git "https://github.com/$REPO.git"
        echo "✔ Installed kotonoha via cargo!"
        exit 0
    else
        echo "❌ Cargo is not installed. Please install Rust/Cargo or wait for binary release."
        exit 1
    fi
fi

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/kotonoha-x86_64-unknown-linux-gnu.tar.gz"

mkdir -p "$INSTALL_DIR"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "📥 Downloading kotonoha $TAG..."
curl -sSL "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR"

install -m 755 "$TMP_DIR/kotonoha" "$INSTALL_DIR/kotonoha"

echo "✔ Successfully installed kotonoha to $INSTALL_DIR/kotonoha!"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "⚠️  Note: $INSTALL_DIR is not in your PATH."
    echo "   Add it to your shell config (~/.bashrc or ~/.zshrc):"
    echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
