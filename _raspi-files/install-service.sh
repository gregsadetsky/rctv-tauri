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

# Create the service file
cat > /etc/systemd/system/rctv-kiosk.service << EOF
[Unit]
Description=RCTV Kiosk
After=network-online.target graphical-session.target
Wants=network-online.target

[Service]
Type=simple
User=pi
Group=pi
Environment=DISPLAY=:0
Environment=WAYLAND_DISPLAY=wayland-0
Environment=XDG_RUNTIME_DIR=/run/user/1000
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

# Reload systemd and enable the service
systemctl daemon-reload
systemctl enable rctv-kiosk.service

echo "Service installed successfully!"
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start rctv-kiosk"
echo "  Stop:    sudo systemctl stop rctv-kiosk"
echo "  Status:  sudo systemctl status rctv-kiosk"
echo "  Logs:    sudo journalctl -u rctv-kiosk -f"
echo ""
echo "The service will start automatically on boot."