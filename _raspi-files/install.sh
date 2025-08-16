#!/bin/bash
# RCTV Kiosk Installation Script for Raspberry Pi
set -e

echo "Installing RCTV Kiosk..."

# Check if running as root
if [[ $EUID -eq 0 ]]; then
   echo "This script should not be run as root. Please run as the pi user." 
   exit 1
fi

# Download latest release
echo "Downloading latest RCTV Kiosk release..."

# Try to get the ARM64 .deb package first (native build)
DEB_URL=$(curl -s https://api.github.com/repos/gregsadetsky/rctv-tauri/releases/latest | grep "browser_download_url.*arm64.*\.deb" | cut -d '"' -f 4)

if [ -n "$DEB_URL" ]; then
    echo "Found ARM64 .deb package: $DEB_URL"
    wget -O /tmp/rctv-kiosk.deb "$DEB_URL"
    echo "Installing .deb package..."
    sudo dpkg -i /tmp/rctv-kiosk.deb || sudo apt-get install -f -y
    rm /tmp/rctv-kiosk.deb
    
    # Find the installed binary
    BINARY_PATH=$(which rctv-tauri || find /usr -name "rctv-tauri" 2>/dev/null | head -1)
    if [ -n "$BINARY_PATH" ]; then
        sudo ln -sf "$BINARY_PATH" /usr/local/bin/rctv-kiosk
        echo "Installation complete! Binary linked to /usr/local/bin/rctv-kiosk"
    else
        echo "Warning: Could not find installed binary"
    fi
else
    echo "No ARM64 .deb found, trying AppImage fallback..."
    # Fallback to AppImage approach
    LATEST_URL=$(curl -s https://api.github.com/repos/gregsadetsky/rctv-tauri/releases/latest | grep "browser_download_url.*AppImage" | cut -d '"' -f 4 | head -1)
    
    if [ -z "$LATEST_URL" ]; then
        echo "Error: Could not find any release files"
        exit 1
    fi
    
    echo "Downloading AppImage from: $LATEST_URL"
    sudo mkdir -p /opt/rctv-kiosk
    sudo wget -O /opt/rctv-kiosk/rctv-kiosk.AppImage "$LATEST_URL"
    sudo chmod +x /opt/rctv-kiosk/rctv-kiosk.AppImage
    sudo ln -sf /opt/rctv-kiosk/rctv-kiosk.AppImage /usr/local/bin/rctv-kiosk
    sudo chown -R pi:pi /opt/rctv-kiosk
    echo "AppImage installation complete!"
fi

# Create log directory
sudo mkdir -p /var/log/rctv-kiosk
sudo chown pi:pi /var/log/rctv-kiosk

echo ""
echo "To start the kiosk manually:"
echo "  rctv-kiosk --token YOUR_TOKEN_HERE"
echo ""
echo "To install as a service, run:"
echo "  sudo ./install-service.sh YOUR_TOKEN_HERE"