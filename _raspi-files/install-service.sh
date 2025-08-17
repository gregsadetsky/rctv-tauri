#!/bin/bash
# RCTV Kiosk Service Installation Script
set -e

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <TOKEN>"
    echo "Example: $0 DEADBEEF-..."
    exit 1
fi

TOKEN=$1

echo "Installing RCTV Kiosk as a system service..."

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root (use sudo)" 
   exit 1
fi

# Get the actual user who called sudo (not root)
ACTUAL_USER=${SUDO_USER:-$USER}
ACTUAL_USER_ID=$(id -u "$ACTUAL_USER")

echo "Setting up service for user: $ACTUAL_USER (ID: $ACTUAL_USER_ID)"

# Create the service file
cat > /etc/systemd/system/rctv-kiosk.service << EOF
[Unit]
Description=RCTV Kiosk
After=network-online.target graphical-session.target
Wants=network-online.target

[Service]
Type=simple
User=$ACTUAL_USER
Group=$ACTUAL_USER
Environment=DISPLAY=:0
Environment=WAYLAND_DISPLAY=wayland-0
Environment=XDG_RUNTIME_DIR=/run/user/$ACTUAL_USER_ID
Environment=WEBKIT_DISABLE_COMPOSITING_MODE=1
ExecStart=/usr/local/bin/rctv-kiosk --token $TOKEN
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical.target
EOF

# Install unclutter for mouse hiding
apt-get update
apt-get install -y unclutter

# Create mouse hiding service
cat > /etc/systemd/system/hide-mouse.service << EOF
[Unit]
Description=Hide mouse cursor
After=graphical-session.target

[Service]
Type=simple
User=$ACTUAL_USER
Group=$ACTUAL_USER
Environment=DISPLAY=:0
ExecStart=/usr/bin/unclutter -idle 0.5
Restart=always
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical.target
EOF

# Reload systemd and enable both services
systemctl daemon-reload
systemctl enable rctv-kiosk.service
systemctl enable hide-mouse.service

echo "Service installed successfully!"
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start rctv-kiosk"
echo "  Stop:    sudo systemctl stop rctv-kiosk"
echo "  Status:  sudo systemctl status rctv-kiosk"
echo "  Logs:    sudo journalctl -u rctv-kiosk -f"
echo ""
echo "The service will start automatically on boot."