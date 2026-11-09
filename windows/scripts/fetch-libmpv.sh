#!/bin/sh
# Downloads libmpv and the libraries it loads (media-kit/libmpv-darwin-build, universal "video-full" build) into
# src-tauri/lib/macos, for dev builds and for the app bundle (Contents/Frameworks).
# That build is mpv with -Dgpl=false and FFmpeg without --enable-gpl: LGPL, like the Windows DLL, which leaves
# Nightglass free to keep its own licence. Pinned: a new version is a change here, checked like any other.
set -eu
VERSION=v0.7.3
ASSET="libmpv-libs_${VERSION}_macos-universal-video-full"
SHA256=f570695d8ba8534835dd4cba0b6e275579c1c22c01a9da0442748ca37e2daac1
lib="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/lib/macos"
if [ -f "$lib/libmpv.dylib" ]; then echo "libmpv.dylib already present"; exit 0; fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $ASSET.tar.gz"
curl -fsSL -o "$tmp/libmpv.tar.gz" "https://github.com/media-kit/libmpv-darwin-build/releases/download/$VERSION/$ASSET.tar.gz"
echo "$SHA256  $tmp/libmpv.tar.gz" | shasum -a 256 -c -
tar -xzf "$tmp/libmpv.tar.gz" -C "$tmp"
mkdir -p "$lib"
cp "$tmp/$ASSET"/*.dylib "$lib/"
# Shipped next to the licences: which build this is and where its sources are.
cat > "$lib/libmpv-version.txt" <<EOF
libmpv.dylib and the libraries next to it: $ASSET (media-kit/libmpv-darwin-build $VERSION)
mpv 0.36.0 (-Dgpl=false) and FFmpeg 6.0 (LGPL, --enable-version3, no --enable-gpl)
Build scripts, source versions and patches of every library: https://github.com/media-kit/libmpv-darwin-build/tree/$VERSION
EOF
echo "libmpv from media-kit/libmpv-darwin-build $VERSION"
