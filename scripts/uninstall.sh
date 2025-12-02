#!/bin/bash

# Network Monitor Uninstallation Script
# Supports both system-wide and local uninstallation
#
# Usage:
#   ./scripts/uninstall.sh          - Local uninstallation
#   sudo ./scripts/uninstall.sh      - System-wide uninstallation
#
# Features:
# - Complete removal of binaries, desktop files, and icons
# - Cache updates for both system and user directories
# - Safe removal with proper error handling

set -e

# Determine installation type and paths
if [ "$EUID" -eq 0 ]; then
    INSTALL_TYPE="system"
    BIN_DIR="/usr/local/bin"
    APPLICATIONS_DIR="/usr/share/applications"
    ICON_DIR="/usr/share/icons/hicolor"
    echo "Uninstalling Network Monitor from system..."
else
    INSTALL_TYPE="local"
    BIN_DIR="$HOME/.local/bin"
    APPLICATIONS_DIR="$HOME/.local/share/applications"
    ICON_DIR="$HOME/.local/share/icons/hicolor"
    echo "Uninstalling Network Monitor from user $USER's local installation..."
fi

# Remove the binary
BINARY_PATH="$BIN_DIR/network-monitor"
if [ -f "$BINARY_PATH" ]; then
    echo "Removing binary: $BINARY_PATH"
    rm -f "$BINARY_PATH"
else
    echo "Warning: Binary not found at $BINARY_PATH"
fi

# Remove desktop file
DESKTOP_FILE="$APPLICATIONS_DIR/network-monitor.desktop"
if [ -f "$DESKTOP_FILE" ]; then
    echo "Removing desktop file: $DESKTOP_FILE"
    rm -f "$DESKTOP_FILE"
else
    echo "Warning: Desktop file not found at $DESKTOP_FILE"
fi

# Remove icons
ICON_SIZES=("16x16" "32x32" "48x48" "64x64" "scalable")
REMOVED_ANY=false

echo "Looking for network-monitor icons in $ICON_DIR..."
find "$ICON_DIR" -name "network-monitor.svg" -exec ls -la {} \;

for size in "${ICON_SIZES[@]}"; do
    ICON_PATH="$ICON_DIR/$size/apps/network-monitor.svg"
    if [ -f "$ICON_PATH" ]; then
        echo "Removing icon: $ICON_PATH"
        rm -f "$ICON_PATH"
        REMOVED_ANY=true
    fi
done

if [ "$REMOVED_ANY" = true ]; then
    # Update icon cache
    echo "Updating icon cache..."
    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || echo "Warning: Could not update icon cache"
    
    # Also update the other icon cache
    if [ "$INSTALL_TYPE" = "system" ] && [ -d "$HOME/.local/share/icons/hicolor" ]; then
        echo "Updating user icon cache..."
        gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
    elif [ "$INSTALL_TYPE" = "local" ] && [ -d "/usr/share/icons/hicolor" ]; then
        echo "Updating system icon cache..."
        gtk-update-icon-cache -f -t "/usr/share/icons/hicolor" 2>/dev/null || true
    fi
fi

# Update desktop database
echo "Updating desktop database..."
update-desktop-database "$APPLICATIONS_DIR" 2>/dev/null || echo "Warning: Could not update desktop database"

# Update the other desktop database as well
if [ "$INSTALL_TYPE" = "system" ]; then
    echo "Updating local desktop database..."
    update-desktop-database ~/.local/share/applications 2>/dev/null || true
else
    echo "Updating system desktop database..."
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi

echo "$INSTALL_TYPE uninstallation complete!"
echo "You may need to restart GNOME Shell (Alt+F2, type 'r', press Enter) to see the changes."