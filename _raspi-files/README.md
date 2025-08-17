# RCTV Kiosk - Raspberry Pi Installation

This directory contains installation scripts for deploying RCTV Kiosk on a Raspberry Pi 5 running Raspberry Pi OS (Debian-based).

## Recommended: Build on Pi (Native)

The simplest and most reliable approach is to build directly on your Raspberry Pi:

1. **One-command install:**
   ```bash
   curl -sSL https://raw.githubusercontent.com/gregsadetsky/rctv-tauri/refs/heads/main/_raspi-files/build-on-pi.sh | bash -s YOUR_TOKEN_HERE
   ```

   Or download and run:
   ```bash
   wget https://raw.githubusercontent.com/gregsadetsky/rctv-tauri/main/_raspi-files/build-on-pi.sh
   chmod +x build-on-pi.sh
   ./build-on-pi.sh YOUR_TOKEN_HERE
   ```

This will:
- Install all dependencies (Rust, Node.js, system libraries)
- Clone and build the application (15-20 minutes)
- Generate signing keys for auto-updates
- Install as a systemd service
- Start automatically

## Alternative: Pre-built Packages

If available, you can try downloading pre-built packages:

1. **Download and run the installer:**
   ```bash
   wget https://raw.githubusercontent.com/gregsadetsky/rctv-tauri/main/_raspi-files/install.sh
   chmod +x install.sh
   ./install.sh
   ```

2. **Install as a system service:**
   ```bash
   wget https://raw.githubusercontent.com/gregsadetsky/rctv-tauri/main/_raspi-files/install-service.sh
   chmod +x install-service.sh
   sudo ./install-service.sh YOUR_TOKEN_HERE
   ```

3. **Start the service:**
   ```bash
   sudo systemctl start rctv-kiosk
   ```

## Manual Installation

If you prefer to install manually:

1. **Download the latest release:**
   ```bash
   sudo mkdir -p /opt/rctv-kiosk
   wget -O /tmp/rctv-kiosk.AppImage [LATEST_RELEASE_URL]
   sudo mv /tmp/rctv-kiosk.AppImage /opt/rctv-kiosk/
   sudo chmod +x /opt/rctv-kiosk/rctv-kiosk.AppImage
   sudo ln -s /opt/rctv-kiosk/rctv-kiosk.AppImage /usr/local/bin/rctv-kiosk
   ```

2. **Test the installation:**
   ```bash
   rctv-kiosk --token YOUR_TOKEN_HERE
   ```

## Service Management

- **Start:** `sudo systemctl start rctv-kiosk`
- **Stop:** `sudo systemctl stop rctv-kiosk`
- **Restart:** `sudo systemctl restart rctv-kiosk`
- **Status:** `sudo systemctl status rctv-kiosk`
- **View logs:** `sudo journalctl -u rctv-kiosk -f`
- **Disable auto-start:** `sudo systemctl disable rctv-kiosk`

## Auto-Updates

The kiosk will automatically check for updates on startup and prompt to install them. Updates are downloaded from GitHub releases and cryptographically verified.

## Troubleshooting

### Display Issues
If the kiosk doesn't appear on screen:
- Ensure X11 is running: `echo $DISPLAY`
- Check if user can access display: `xhost +local:`

### Service Won't Start
- Check logs: `sudo journalctl -u rctv-kiosk -f`
- Verify token is correct
- Ensure network connectivity

### Manual Update
If auto-update fails, manually update:
```bash
./install.sh  # Re-run installer to get latest version
sudo systemctl restart rctv-kiosk
```

## Kiosk Setup Recommendations

For a proper kiosk setup, consider:

1. **Auto-login to desktop** (use `raspi-config`)
2. **Disable screen blanking:**
   ```bash
   # Add to /home/pi/.bashrc
   export DISPLAY=:0
   xset s off -dpms
   ```
3. **Hide desktop background** and taskbar
4. **Disable WiFi power management:**
   ```bash
   sudo iwconfig wlan0 power off
   ```

## Files

- `install.sh` - Downloads and installs the latest RCTV Kiosk release
- `install-service.sh` - Creates and enables the systemd service
- `README.md` - This documentation