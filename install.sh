#!/bin/bash
set -e

REPO="prashantdubey/terrek"
BINARY="terrek"

OS="$(uname -s)"

if [[ "$OS" == "Darwin" ]]; then
  FILE="terrek-macos-latest"
elif [[ "$OS" == "Linux" ]]; then
  FILE="terrek-ubuntu-latest"
else
  echo "Unsupported OS"
  exit 1
fi

URL="https://github.com/$REPO/releases/latest/download/$FILE"

echo "Downloading $BINARY..."
curl -L $URL -o $BINARY

chmod +x $BINARY
sudo mv $BINARY /usr/local/bin/$BINARY

echo "Installed successfully!"
