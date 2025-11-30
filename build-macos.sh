#!/bin/bash
# Build script for network-monitor on macOS
# This script installs the necessary homebrew dependencies and builds the project

set -e

echo "==> Installing GTK4 and libadwaita via Homebrew..."
echo "Note: This may take 10-30 minutes depending on your system"

# Install GTK4 and dependencies
brew install gtk4 libadwaita

echo ""
echo "==> Setting up PKG_CONFIG_PATH..."

# Set PKG_CONFIG_PATH to find GTK4 libraries
export PKG_CONFIG_PATH="/usr/local/opt/gtk4/lib/pkgconfig:/usr/local/opt/libadwaita/lib/pkgconfig:$PKG_CONFIG_PATH"

echo "PKG_CONFIG_PATH=$PKG_CONFIG_PATH"
echo ""
echo "==> Building network-monitor..."

# Build the project
cargo build --release

echo ""
echo "==> Build complete!"
echo "Run the application with: ./target/release/network-monitor"
echo ""
echo "Note: Add the following to your ~/.zshrc to make PKG_CONFIG_PATH permanent:"
echo '  export PKG_CONFIG_PATH="/usr/local/opt/gtk4/lib/pkgconfig:/usr/local/opt/libadwaita/lib/pkgconfig:$PKG_CONFIG_PATH"'
