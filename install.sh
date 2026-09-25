#!/bin/sh
# Terrek installer: curl -fsSL https://raw.githubusercontent.com/prashantxy/Terrek/main/install.sh | sh
set -eu

REPO="prashantxy/Terrek"
INSTALL_DIR="${TERREK_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)
case "$os-$arch" in
  Darwin-arm64)              target="aarch64-apple-darwin" ;;
  Darwin-x86_64)             target="x86_64-apple-darwin" ;;
  Linux-x86_64)              target="x86_64-unknown-linux-musl" ;;
  Linux-aarch64|Linux-arm64) target="aarch64-unknown-linux-musl" ;;
  *) echo "terrek: no prebuilt binary for $os $arch; install with: cargo install --git https://github.com/$REPO" >&2; exit 1 ;;
esac

archive="terrek-$target.tar.gz"
base="https://github.com/$REPO/releases/latest/download"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $archive…"
curl -fsSL "$base/$archive" -o "$tmp/$archive"
curl -fsSL "$base/$archive.sha256" -o "$tmp/$archive.sha256"

expected=$(cut -d ' ' -f 1 < "$tmp/$archive.sha256")
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$tmp/$archive" | cut -d ' ' -f 1)
else
  actual=$(shasum -a 256 "$tmp/$archive" | cut -d ' ' -f 1)
fi
if [ "$expected" != "$actual" ]; then
  echo "terrek: checksum mismatch, refusing to install" >&2
  exit 1
fi

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/terrek" "$INSTALL_DIR/terrek"
echo "Installed terrek to $INSTALL_DIR/terrek"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Add it to your PATH:  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
echo "Next: run \`terrek setup\`, then \`terrek\`."
