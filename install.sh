#!/bin/sh
set -eu

REPO="guyco3/memblocks"
ARTIFACT="memblocks-macos.tar.gz"
VERSION="${MEMBLOCKS_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/${ARTIFACT}"
else
  URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"
fi

echo "Downloading ${URL}"
curl -fsSL -o "$TMP_DIR/$ARTIFACT" "$URL"

tar -xzf "$TMP_DIR/$ARTIFACT" -C "$TMP_DIR"

if [ ! -f "$TMP_DIR/memblocks" ]; then
  echo "Error: memblocks binary not found in release artifact." >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP_DIR/memblocks" "$INSTALL_DIR/memblocks"

echo "Installed memblocks to $INSTALL_DIR/memblocks"

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo ""
    echo "Add this to your shell profile so memblocks is on PATH:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo ""
echo "Try: memblocks /"
