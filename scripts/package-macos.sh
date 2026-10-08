#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
    echo 'This script requires macOS, Xcode Command Line Tools, and Rust.' >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1 && [[ -x .build-tools/cargo/bin/cargo ]]; then
    export CARGO_HOME="$PWD/.build-tools/cargo"
    export RUSTUP_HOME="$PWD/.build-tools/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi

TARGET="${1:-aarch64-apple-darwin}"
case "$TARGET" in
    aarch64-apple-darwin) ARCH=arm64; export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}" ;;
    x86_64-apple-darwin) ARCH=x86_64; export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-10.13}" ;;
    *) echo "Unsupported target: $TARGET" >&2; exit 1 ;;
esac
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
cargo build --release --locked --target "$TARGET"

mkdir -p dist
STAGING=$(mktemp -d "$PWD/dist/staging.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
APP="$STAGING/escpresso.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/$TARGET/release/escpresso" "$APP/Contents/MacOS/escpresso"
cp LICENSE "$APP/Contents/Resources/LICENSE"
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>CFBundleName</key><string>escpresso</string>
    <key>CFBundleDisplayName</key><string>escpresso</string>
    <key>CFBundleIdentifier</key><string>com.github.jflaflamme.escpresso</string>
    <key>CFBundleExecutable</key><string>escpresso</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSLocalNetworkUsageDescription</key><string>Accept ESC/POS receipt data from POS clients on your local network.</string>
</dict></plist>
EOF
plutil -lint "$APP/Contents/Info.plist"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"
ln -s /Applications "$STAGING/Applications"
cp LICENSE "$STAGING/LICENSE.txt"
cat > "$STAGING/README.txt" <<EOF
escpresso $VERSION — $ARCH

Drag escpresso.app into Applications, then open it.
The emulator accepts ESC/POS data on TCP port 9100.
This local build uses ad-hoc signing and has not been notarized by Apple.
If macOS blocks a downloaded copy, use System Settings > Privacy & Security > Open Anyway.

Source: https://github.com/heiseshandian/escpresso
Source commit: $(git rev-parse HEAD)
License: MIT (included)
EOF
DMG="$PWD/dist/escpresso-$VERSION-macos-$ARCH.dmg"
if [[ -e "$DMG" ]]; then
    echo "Output already exists: $DMG. Move it aside before rebuilding." >&2
    exit 1
fi
hdiutil create -volname "escpresso $VERSION" -srcfolder "$STAGING" -format UDZO "$DMG"
hdiutil verify "$DMG"
echo "Created $DMG"
