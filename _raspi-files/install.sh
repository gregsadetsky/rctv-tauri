#!/bin/bash
# RCTV Kiosk Installation Script for Raspberry Pi
set -e

echo "Installing RCTV Kiosk..."

# Check if running as root
if [[ $EUID -eq 0 ]]; then
   echo "This script should not be run as root. Please run as the pi user." 
   exit 1
fi

# Create directories
sudo mkdir -p /opt/rctv-kiosk
sudo mkdir -p /var/log/rctv-kiosk

# Download latest release
echo "Downloading latest RCTV Kiosk release..."
LATEST_URL=$(curl -s https://api.github.com/repos/gregsadetsky/rctv-tauri/releases/latest | grep "browser_download_url.*aarch64.*AppImage" | cut -d '"' -f 4)

if [ -z "$LATEST_URL" ]; then
    echo "Error: Could not find latest release download URL"
    exit 1
fi

echo "Downloading from: $LATEST_URL"
sudo wget -O /opt/rctv-kiosk/rctv-kiosk.AppImage "$LATEST_URL"
sudo chmod +x /opt/rctv-kiosk/rctv-kiosk.AppImage

# Create symlink for easy access
sudo ln -sf /opt/rctv-kiosk/rctv-kiosk.AppImage /usr/local/bin/rctv-kiosk

# Set ownership
sudo chown -R pi:pi /opt/rctv-kiosk
sudo chown pi:pi /var/log/rctv-kiosk

echo "Installation complete!"
echo ""
echo "To start the kiosk manually:"
echo "  rctv-kiosk --token YOUR_TOKEN_HERE"
echo ""
echo "To install as a service, run:"
echo "  sudo ./install-service.sh YOUR_TOKEN_HERE"