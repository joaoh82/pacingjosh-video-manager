#!/usr/bin/env bash
# Fetch FFmpeg / FFprobe sidecar binaries for the Tauri bundle.
#
# Downloads one target at a time based on the host platform — cross-targets
# can be added by re-running with an env var override (see TARGET_TRIPLE below).
#
# Usage:
#   bash scripts/fetch-ffmpeg.sh               # auto-detect host triple
#   TARGET_TRIPLE=x86_64-apple-darwin bash scripts/fetch-ffmpeg.sh

set -euo pipefail

BIN_DIR="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"
mkdir -p "$BIN_DIR"

# Detect host target triple (override with TARGET_TRIPLE)
if [[ -z "${TARGET_TRIPLE:-}" ]]; then
    if command -v rustc >/dev/null 2>&1; then
        TARGET_TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
    else
        echo "rustc not found — set TARGET_TRIPLE manually and re-run" >&2
        exit 1
    fi
fi
echo "Target: $TARGET_TRIPLE"

case "$TARGET_TRIPLE" in
    x86_64-unknown-linux-gnu)
        URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
        EXT="tar.xz"
        FFMPEG_NAME="ffmpeg"
        FFPROBE_NAME="ffprobe"
        ;;
    aarch64-unknown-linux-gnu)
        URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
        EXT="tar.xz"
        FFMPEG_NAME="ffmpeg"
        FFPROBE_NAME="ffprobe"
        ;;
    x86_64-apple-darwin|aarch64-apple-darwin)
        echo "macOS: download manually from https://evermeet.cx/ffmpeg/ (LGPL variant)"
        echo "Place ffmpeg and ffprobe as:"
        echo "  $BIN_DIR/ffmpeg-$TARGET_TRIPLE"
        echo "  $BIN_DIR/ffprobe-$TARGET_TRIPLE"
        exit 0
        ;;
    x86_64-pc-windows-msvc|aarch64-pc-windows-msvc)
        echo "Windows: use scripts/fetch-ffmpeg.ps1 instead"
        exit 1
        ;;
    *)
        echo "Unsupported target triple: $TARGET_TRIPLE" >&2
        exit 1
        ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $URL..."
curl -fL "$URL" -o "$TMP/ffmpeg.$EXT"

echo "Extracting..."
case "$EXT" in
    tar.xz) tar -xJf "$TMP/ffmpeg.$EXT" -C "$TMP" ;;
    zip)    unzip -q "$TMP/ffmpeg.$EXT" -d "$TMP" ;;
esac

FFMPEG_PATH="$(find "$TMP" -type f -name "$FFMPEG_NAME" | head -n1)"
FFPROBE_PATH="$(find "$TMP" -type f -name "$FFPROBE_NAME" | head -n1)"

if [[ -z "$FFMPEG_PATH" || -z "$FFPROBE_PATH" ]]; then
    echo "Failed to locate ffmpeg/ffprobe inside archive" >&2
    exit 1
fi

cp "$FFMPEG_PATH"  "$BIN_DIR/ffmpeg-$TARGET_TRIPLE"
cp "$FFPROBE_PATH" "$BIN_DIR/ffprobe-$TARGET_TRIPLE"
chmod +x "$BIN_DIR/ffmpeg-$TARGET_TRIPLE" "$BIN_DIR/ffprobe-$TARGET_TRIPLE"

echo "Installed:"
ls -lh "$BIN_DIR/ffmpeg-$TARGET_TRIPLE" "$BIN_DIR/ffprobe-$TARGET_TRIPLE"
