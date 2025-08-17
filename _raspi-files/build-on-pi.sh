#!/bin/bash
# RCTV Kiosk Build and Install Script for Raspberry Pi
set -e

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <TOKEN>"
    echo "Example: $0 DEADBEEF-..."
    exit 1
fi

TOKEN=$1

echo "=== RCTV Kiosk Build and Install on Raspberry Pi ==="
echo "This will install dependencies, build the app, and set up the service."
echo ""

# Check if running as root (we want to avoid this)
if [[ $EUID -eq 0 ]]; then
   echo "This script should not be run as root."
   echo "Please run as a regular user (like rctv, pi, etc.)"
   exit 1
fi

echo "1. Installing system dependencies..."
sudo apt-get update
sudo apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    pkg-config \
    libasound2-dev

echo "2. Installing Node.js..."
if ! command -v node &> /dev/null; then
    curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
    sudo apt-get install -y nodejs
else
    echo "Node.js already installed: $(node --version)"
fi

echo "3. Installing Rust..."
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo "Rust already installed: $(cargo --version)"
fi

echo "4. Cloning RCTV source code..."
cd /tmp
rm -rf rctv-tauri
git clone https://github.com/gregsadetsky/rctv-tauri.git
cd rctv-tauri

echo "5. Installing Node.js dependencies..."
npm install

echo "6. Building application (this may take 15-20 minutes)..."
echo "Building with signing keys for auto-updates..."

# Set up signing environment from files
# IMPORTANT: This script now assumes two files exist:
# 1. The private key at ~/.tauri/rctv-kiosk.key
# 2. The password for the key in a file at ~/.tauri/rctv-kiosk.password

PRIVATE_KEY_PATH="$HOME/.tauri/rctv-kiosk.key"
PASSWORD_PATH="$HOME/.tauri/rctv-kiosk.password"

if [ ! -f "$PRIVATE_KEY_PATH" ]; then
    echo "ERROR: Signing key not found at $PRIVATE_KEY_PATH"
    echo "Please create it and the password file before running this script."
    exit 1
fi

if [ ! -f "$PASSWORD_PATH" ]; then
    echo "ERROR: Signing key password file not found at $PASSWORD_PATH"
    echo "Please create it before running this script."
    exit 1
fi

echo "Loading signing key and password from files..."
export TAURI_SIGNING_PRIVATE_KEY=$(cat "$PRIVATE_KEY_PATH")
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(cat "$PASSWORD_PATH")

# Build the application
npm run tauri build

echo "7. Installing the built application..."
# Find the built .deb file
DEB_FILE=$(find src-tauri/target/release/bundle/deb -name "*.deb" | head -1)

if [ -z "$DEB_FILE" ]; then
    echo "No .deb file found, trying AppImage..."
    APPIMAGE_FILE=$(find src-tauri/target/release/bundle/appimage -name "*.AppImage" | head -1)
    
    if [ -z "$APPIMAGE_FILE" ]; then
        echo "No built files found! Build may have failed."
        exit 1
    fi
    
    # Install AppImage
    sudo mkdir -p /opt/rctv-kiosk
    sudo cp "$APPIMAGE_FILE" /opt/rctv-kiosk/rctv-kiosk.AppImage
    sudo chmod +x /opt/rctv-kiosk/rctv-kiosk.AppImage
    sudo ln -sf /opt/rctv-kiosk/rctv-kiosk.AppImage /usr/local/bin/rctv-kiosk
    echo "Installed AppImage to /opt/rctv-kiosk/"
else
    # Install .deb package
    echo "Installing .deb package: $DEB_FILE"
    sudo dpkg -i "$DEB_FILE" || sudo apt-get install -f -y
    
    # Find the installed binary and link it
    BINARY_PATH=$(which rctv-tauri || find /usr -name "rctv-tauri" 2>/dev/null | head -1)
    if [ -n "$BINARY_PATH" ]; then
        sudo ln -sf "$BINARY_PATH" /usr/local/bin/rctv-kiosk
        echo "Installed .deb package and linked binary"
    fi
fi

echo "8. Setting up systemd service..."
USER_ID=$(id -u)
sudo tee /etc/systemd/system/rctv-kiosk.service > /dev/null << EOF
[Unit]
Description=RCTV Kiosk
After=network-online.target graphical-session.target
Wants=network-online.target

[Service]
Type=simple
User=$USER
Group=$USER
Environment=DISPLAY=:0
Environment=WAYLAND_DISPLAY=wayland-0
Environment=XDG_RUNTIME_DIR=/run/user/$USER_ID
Environment="WEBKIT_DISABLE_DMABUF_RENDERER=1"
ExecStart=/usr/local/bin/rctv-kiosk --token $TOKEN
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

# Disable screen blanking for kiosk
ExecStartPre=/bin/bash -c 'export DISPLAY=:0 && xset s off -dpms || true'

[Install]
WantedBy=graphical.target
EOF

# Create log directory
sudo mkdir -p /var/log/rctv-kiosk
sudo chown $USER:$USER /var/log/rctv-kiosk

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable rctv-kiosk.service

echo ""
echo "=== Installation Complete! ==="
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start rctv-kiosk"
echo "  Stop:    sudo systemctl stop rctv-kiosk"
echo "  Status:  sudo systemctl status rctv-kiosk"
echo "  Logs:    sudo journalctl -u rctv-kiosk -f"
echo ""
echo "Test manually first:"
echo "  rctv-kiosk --token $TOKEN"
echo ""
echo "The service will start automatically on boot."
echo ""
echo "Your signing keys are in ~/.tauri/ - keep them safe for future builds!"