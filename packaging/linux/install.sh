#!/usr/bin/env bash
set -euo pipefail

REPO="dmoliveira/skill"
VERSION="${VERSION:-latest}"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64)
    ARCH="x86_64"
    ;;
  aarch64|arm64)
    ARCH="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

ASSET="skill-${ARCH}-unknown-linux-gnu.tar.gz"
if [[ "$VERSION" == "latest" ]]; then
  URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"
else
  URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ASSET}"
fi

curl -fsSL "$URL" | tar -xz
sudo install skill /usr/local/bin/skill
echo "Installed skill to /usr/local/bin/skill"
