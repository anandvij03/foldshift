#!/usr/bin/env bash
# FoldShift Installer for macOS and Linux
set -euo pipefail

REPO="anandvij03/foldshift"
PROFILE="${FOLDSHIFT_PROFILE:-web}" # "core", "images", or "web"
VERSION="${FOLDSHIFT_VERSION:-latest}"
INSTALL_DIR="${FOLDSHIFT_INSTALL_DIR:-$HOME/.local/bin}"

echo "=================================================="
echo " FoldShift Installer for Unix / macOS"
echo " Profile: $PROFILE | Version: $VERSION"
echo " Target dir: $INSTALL_DIR"
echo "=================================================="

# Detect OS
OS_TYPE="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH_TYPE="$(uname -m)"

case "$OS_TYPE" in
  linux*)
    TARGET_OS="unknown-linux-gnu"
    ;;
  darwin*)
    TARGET_OS="apple-darwin"
    ;;
  *)
    echo "Unsupported OS: $OS_TYPE" >&2
    exit 1
    ;;
esac

case "$ARCH_TYPE" in
  x86_64|amd64)
    TARGET_ARCH="x86_64"
    ;;
  arm64|aarch64)
    if [ "$OS_TYPE" = "darwin" ]; then
      TARGET_ARCH="aarch64"
    else
      TARGET_ARCH="x86_64" # fallback or emulated
    fi
    ;;
  *)
    echo "Unsupported architecture: $ARCH_TYPE" >&2
    exit 1
    ;;
esac

TARGET="${TARGET_ARCH}-${TARGET_OS}"

# Resolve version
if [ "$VERSION" = "latest" ]; then
  TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || echo "v0.1.0")
  CLEAN_VER="${TAG#v}"
else
  CLEAN_VER="${VERSION#v}"
  TAG="v${CLEAN_VER}"
fi

ARCHIVE_NAME="foldshift-${PROFILE}-${CLEAN_VER}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading ${ARCHIVE_NAME}..."
if ! curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}"; then
  echo "Error downloading from $DOWNLOAD_URL." >&2
  echo "Verify that the release exists at https://github.com/$REPO/releases" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
echo "Extracting to $INSTALL_DIR..."
tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$INSTALL_DIR"

# Ensure binary is executable
if [ "$PROFILE" = "web" ]; then
  chmod +x "$INSTALL_DIR/foldshift-web" 2>/dev/null || true
  BIN_NAME="foldshift-web"
else
  chmod +x "$INSTALL_DIR/foldshift" 2>/dev/null || true
  BIN_NAME="foldshift"
fi

echo ""
echo "Foldshift successfully installed to $INSTALL_DIR/$BIN_NAME"
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo "Note: Make sure to add $INSTALL_DIR to your PATH:"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
echo ""
echo "Run:"
if [ "$PROFILE" = "web" ]; then
  echo "  $BIN_NAME"
  echo "  Then visit http://localhost:3000 in your browser."
else
  echo "  $BIN_NAME --help"
fi
