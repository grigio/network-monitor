#!/bin/bash

# Network Monitor Installation Script
# Supports both system-wide and local installation
# 
# Usage:
#   ./scripts/install.sh          - Local installation (no sudo required)
#   sudo ./scripts/install.sh      - System-wide installation
#
# Features:
# - Installs both GTK4 and TUI binaries
# - Proper WM class setting for GNOME dock pinning
# - Desktop file with correct Exec path
# - Icon installation and cache updates
# - Dual installation mode support

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Determine installation type
if [ "$EUID" -eq 0 ]; then
    INSTALL_TYPE="system"
    BIN_DIR="/usr/local/bin"
    APPLICATIONS_DIR="/usr/share/applications"
    ICON_DIR="/usr/share/icons/hicolor"
    echo "Installing Network Monitor system-wide..."
else
    INSTALL_TYPE="local"
    BIN_DIR="$HOME/.local/bin"
    APPLICATIONS_DIR="$HOME/.local/share/applications"
    ICON_DIR="$HOME/.local/share/icons/hicolor"
    echo "Installing Network Monitor locally for user $USER..."
fi

# Build the binaries
echo "Building binaries..."
if [ "$INSTALL_TYPE" = "system" ]; then
    sudo -E RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup} /usr/bin/cargo build --release
    GTK_BINARY_PATH="target/release/network-monitor"
    TUI_BINARY_PATH="target/release/nmt"
else
    cargo build
    GTK_BINARY_PATH="target/debug/network-monitor"
    TUI_BINARY_PATH="target/debug/nmt"
fi

# Install binaries
echo "Installing binaries to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp "$GTK_BINARY_PATH" "$BIN_DIR/"
chmod 755 "$BIN_DIR/network-monitor"
cp "$TUI_BINARY_PATH" "$BIN_DIR/"
chmod 755 "$BIN_DIR/nmt"

# Install desktop file with correct Exec path
DESKTOP_FILE="network-monitor.desktop"

if [ -f "$DESKTOP_FILE" ]; then
    echo "Installing desktop file to $APPLICATIONS_DIR..."
    mkdir -p "$APPLICATIONS_DIR"
    
    # Use appropriate binary location
    EXEC_PATH="$BIN_DIR/network-monitor"
    
    # Update Exec line in desktop file and ensure GNOME Shell compatibility
    sed "s|^Exec=.*|Exec=$EXEC_PATH|" "$DESKTOP_FILE" > "$APPLICATIONS_DIR/$DESKTOP_FILE"
    
    # Ensure proper Categories (single main category for GNOME Shell)
    sed -i 's|Categories=System;Network;Monitor;|Categories=System;|' "$APPLICATIONS_DIR/$DESKTOP_FILE"
    
    # Add required keys for GNOME Shell visibility
    grep -q "^NoDisplay=false" "$APPLICATIONS_DIR/$DESKTOP_FILE" || echo "NoDisplay=false" >> "$APPLICATIONS_DIR/$DESKTOP_FILE"
    grep -q "^DBusActivatable=false" "$APPLICATIONS_DIR/$DESKTOP_FILE" || echo "DBusActivatable=false" >> "$APPLICATIONS_DIR/$DESKTOP_FILE"
    
    # Set permissions
    chmod 644 "$APPLICATIONS_DIR/$DESKTOP_FILE"
else
    echo "Error: $DESKTOP_FILE not found!"
    exit 1
fi

# Install icons
if [ -d "icons" ]; then
    echo "Installing icons to $ICON_DIR..."
    echo "Source: icons/hicolor/"
    echo "Destination: $ICON_DIR"
    
    # Ensure target directory exists
    mkdir -p "$ICON_DIR"
    
    # Copy icons with verbose output
    cp -rv icons/hicolor/* "$ICON_DIR/"
    
    # Verify icons were copied
    echo "Verifying icon installation..."
    find "$ICON_DIR" -name "network-monitor.svg" -exec ls -la {} \;
    
    # Update icon cache
    echo "Updating icon cache..."
    gtk-update-icon-cache -f -t "$ICON_DIR"
    
    # Also update system icon cache if doing local installation
    if [ "$INSTALL_TYPE" = "local" ] && [ -d "/usr/share/icons/hicolor" ]; then
        echo "Updating system icon cache..."
        gtk-update-icon-cache -f -t "/usr/share/icons/hicolor" 2>/dev/null || true
    fi
else
    echo "Error: icons directory not found!"
    exit 1
fi

# Update desktop database
echo "Updating desktop database..."
update-desktop-database "$APPLICATIONS_DIR"

# Update the other desktop database as well
if [ "$INSTALL_TYPE" = "system" ]; then
    echo "Updating local desktop database..."
    update-desktop-database ~/.local/share/applications/ 2>/dev/null || true
else
    echo "Updating system desktop database..."
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi

echo "$INSTALL_TYPE installation complete!"
echo ""
echo "📦 Installed binaries:"
echo "  - network-monitor (GTK4 GUI)"
echo "  - nmt (Terminal UI)"
echo ""
echo "⚠️  IMPORTANT: To see the correct icon in GNOME Shell:"
echo "1. Restart GNOME Shell (Alt+F2, type 'r', press Enter)"
echo "2. Or log out and log back in"
echo ""
echo "This ensures the icon cache is refreshed and your custom icon appears."
echo ""
echo "The GTK4 application should now be pinnable to the GNOME dock/dashboard."
echo "You can run 'nmt' from any terminal to use the TUI version."