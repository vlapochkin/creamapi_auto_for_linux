#!/bin/bash

# Ensure the script stops on errors
set -e

echo "Starting installation for CreamAPI Auto..."

# Build the project
echo "Building the project..."
cargo build --release

# Ensure directories exist
mkdir -p ~/.local/bin
mkdir -p ~/.local/share/applications

# Copy the binary
echo "Installing binary to ~/.local/bin..."
cp target/release/cream-api-auto ~/.local/bin/

# Copy the desktop file
echo "Installing desktop entry..."
cp creamapi-auto.desktop ~/.local/share/applications/

# Update desktop database
echo "Updating desktop database..."
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database ~/.local/share/applications/
else
    echo "update-desktop-database not found. Skipping."
fi

echo "Installation complete! You can now run 'CreamAPI Auto' from your application menu."
