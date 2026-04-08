#!/bin/bash

set -e

APP_NAME="cream-api-auto"
BIN_NAME="cream-api-auto"
APP_DIR="AppDir"

echo "Building $APP_NAME..."
cargo build --release

echo "Preparing AppDir..."
rm -rf $APP_DIR
mkdir -p $APP_DIR/usr/bin
mkdir -p $APP_DIR/usr/share/applications
mkdir -p $APP_DIR/usr/share/icons/hicolor/scalable/apps

# Copy binary and resources
# Download SmokeAPI core if missing
if [ ! -d "resources" ] || [ -z "$(ls -A resources)" ]; then
    echo "Resources missing. Downloading SmokeAPI..."
    mkdir -p resources
    # Use the same logic as updater: download from GitHub
    LATEST_RELEASE=$(curl -s https://api.github.com/repos/acidicoala/SmokeAPI/releases/latest)
    DOWNLOAD_URL=$(echo $LATEST_RELEASE | grep -o 'https://github.com/acidicoala/SmokeAPI/releases/download/[^"]*.zip' | head -n 1)
    wget -q -O smokeapi.zip "$DOWNLOAD_URL"
    unzip -o smokeapi.zip -d resources
    rm smokeapi.zip
    # Get tag name for version.txt
    TAG_NAME=$(echo $LATEST_RELEASE | grep -o '"tag_name": "[^"]*"' | head -n 1 | cut -d'"' -f4)
    echo "$TAG_NAME" > resources/version.txt
fi

cp target/release/$BIN_NAME $APP_DIR/usr/bin/
cp -r resources $APP_DIR/usr/bin/

# Copy desktop file and icon
cp $BIN_NAME.desktop $APP_DIR/usr/share/applications/
# Use a generic icon for now, or you can provide a real one
# cp icon.svg $APP_DIR/usr/share/icons/hicolor/scalable/apps/$APP_NAME.svg

# Download linuxdeploy and gtk plugin if not present
if [ ! -f linuxdeploy-x86_64.AppImage ]; then
    echo "Downloading linuxdeploy..."
    wget -q https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
    chmod +x linuxdeploy-x86_64.AppImage
fi

if [ ! -f linuxdeploy-plugin-gtk-x86_64.AppImage ]; then
    echo "Downloading linuxdeploy-plugin-gtk..."
    wget -q https://github.com/linuxdeploy/linuxdeploy-plugin-gtk/releases/download/continuous/linuxdeploy-plugin-gtk-x86_64.AppImage
    chmod +x linuxdeploy-plugin-gtk-x86_64.AppImage
fi

# Run linuxdeploy
export OUTPUT="$APP_NAME-x86_64.AppImage"
./linuxdeploy-x86_64.AppImage --appdir $APP_DIR \
    --plugin gtk \
    --output appimage \
    --desktop-file $APP_DIR/usr/share/applications/$BIN_NAME.desktop \
    --executable $APP_DIR/usr/bin/$BIN_NAME

echo "AppImage created: $OUTPUT"
