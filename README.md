# Auto Captive Portal (ACP)

A Rust-based daemon that automatically authenticates against IIT Mandi's captive portal. It runs as a background service with intelligent hybrid network monitoring (netwatcher + adaptive polling), secure credential storage, and desktop notifications.

## Features

- **🚀 Automatic Login**: Detects and authenticates with the captive portal instantly upon network changes
- **🔄 Hybrid Monitoring**: Combines real-time network event detection with intelligent exponential backoff polling
- **🔐 Secure Credentials**: Stores credentials in OS keychain (macOS Keychain / Linux Secret Service / Windows Credential Manager)
- **🔔 Desktop Notifications**: Get notified when successfully logged in
- **⚡ Smart Retry Logic**: Automatic retry with exponential backoff on login failures
- **📊 Service Status**: Monitor service health and login statistics
- **🎯 Cross-Platform**: Supports macOS (x86_64, ARM64), Linux (x86_64), and Windows (x86_64)

## Prerequisites

- **macOS**, **Linux**, or **Windows**
- **jq** (required for the installation script on macOS/Linux):
  - On macOS: `brew install jq`
  - On Linux: `apt install jq` (Debian/Ubuntu) or `yum install jq` (CentOS/RHEL)
- **Rust and Cargo** (only if building from source)

## Installation

### macOS / Linux

To install and set up the Auto Captive Portal Login service, run:

```bash
curl -fsSL https://raw.githubusercontent.com/amansikarwar/auto-captive-portal/main/install.sh | bash
```

This command will:

1. Download the latest `acp` binary for your platform from GitHub releases
2. Install it to `/usr/local/bin/acp`
3. Prompt you for your LDAP credentials
4. Store credentials securely in your OS keychain
5. Create and start the background service

**Note**: You will be prompted for your LDAP credentials during the setup process.

### Windows

1. Download the latest `acp-script-windows-amd64.exe` from [GitHub Releases](https://github.com/amansikarwar/auto-captive-portal/releases)
2. Rename it to `acp.exe` and place it in a permanent location (e.g., `C:\Program Files\ACP\`)
3. Run as Administrator: `acp.exe setup`

The Windows version includes a UAC manifest that automatically requests administrator privileges when installing the service.

### Supported Platforms

- **Linux (x86_64)** - Ubuntu, Debian, Fedora, CentOS, etc.
- **macOS (x86_64)** - Intel-based Macs
- **macOS (ARM64)** - Apple Silicon Macs (M1, M2, M3, etc.)
- **Windows (x86_64)** - Windows 10/11

## Usage

### Commands

```bash
# Show service status and statistics
acp status

# Update stored credentials
acp update-credentials

# Perform health check (verify credentials, portal detection, connectivity)
acp health

# Logout from captive portal
acp logout

# Logout and clear stored credentials
acp logout --clear-credentials

# Show help and available commands
acp --help

# Run daemon directly (for testing)
acp run
```

### Windows Service Commands

```powershell
# Install Windows service (run as Administrator)
acp.exe service install

# Uninstall Windows service
acp.exe service uninstall

# Start/stop the service
acp.exe service start
acp.exe service stop
```

### Service Status Example

The `acp status` command provides comprehensive information:

```text
╔══════════════════════════════════════════════════════╗
║     Auto Captive Portal - Service Status             ║
╚══════════════════════════════════════════════════════╝

Credentials:        ✓ Configured (user: your_username)
Service:            ✓ Running
Internet:           ✓ Connected
Portal Status:      ✓ Not detected

─────────────────────────────────────────────────────
Last Check:         2 minutes ago
Last Login:         15 minutes ago
Last Portal:        https://login.iitmandi.ac.in:1003/portal
```

## Platform-specific Details

### macOS

The service runs as a **LaunchAgent** (`com.user.acp`) and starts automatically on login.

**Manual Service Management:**

```bash
# Check if service is running
launchctl list | grep com.user.acp

# Start service
launchctl load ~/Library/LaunchAgents/com.user.acp.plist

# Stop service
launchctl unload ~/Library/LaunchAgents/com.user.acp.plist

# View logs (last 5 minutes)
log show --predicate 'processImagePath contains "acp"' --last 5m

# Follow logs in real-time
log stream --predicate 'processImagePath contains "acp"'
```

**Credentials Storage:** Uses macOS Keychain (`security` command)

**Log File Location:** `~/.local/share/acp/logs/acp.log`

### Linux

The service runs as a **systemd user service** (`acp.service`).

**Manual Service Management:**

```bash
# Check service status
systemctl --user status acp

# Start service
systemctl --user start acp

# Stop service
systemctl --user stop acp

# Restart service
systemctl --user restart acp

# View logs (last 50 lines)
journalctl --user -u acp -n 50

# Follow logs in real-time
journalctl --user -u acp -f
```

**Credentials Storage:** Uses Linux Secret Service (`libsecret`)

**Log File Location:** `~/.local/share/acp/logs/acp.log`

### Windows

The service runs as a **Windows Service** (`acp`) and can be configured to start automatically.

**Manual Service Management:**

```powershell
# Check service status
sc query acp

# Start service
net start acp

# Stop service
net stop acp

# View service in Services Manager
services.msc
```

**Credentials Storage:** Uses Windows Credential Manager

**Log File Location:** `%APPDATA%\acp\logs\acp.log`

**Note:** Installing or uninstalling the Windows service requires Administrator privileges. The application will automatically prompt for elevation via UAC.

## How It Works

### Network Monitoring Architecture

ACP uses a **hybrid monitoring approach** for optimal responsiveness and resource efficiency:

1. **Real-time Network Event Detection** (via `netwatcher` library)
   - Monitors network interface changes (new interfaces, IP address assignments)
   - Triggers immediate portal check (after 3-second debounce delay)
   - Detects: Wi-Fi connections, VPN changes, ethernet connections

2. **Adaptive Polling** (exponential backoff)
   - **Portal detected**: Checks every 10 seconds
   - **Successfully logged in**: Checks every 30 minutes (1800 seconds)
   - **No portal found**: Gradually decreases interval (exponential decay to 10s minimum)

### Authentication Flow

1. **Portal Detection**: Requests `http://clients3.google.com/generate_204`
   - No portal: Receives 204 response → Internet accessible
   - Portal present: Receives 200 redirect → Extract portal URL and magic value

2. **Login Attempt**: POSTs to `https://login.iitmandi.ac.in:1003/portal?` with:
   - Username and password (from keychain)
   - Magic value (extracted from portal HTML)
   - Redirect URL

3. **Verification**: Confirms internet connectivity after login
   - Success → Desktop notification + Set 30-minute poll interval
   - Failure → Retry with exponential backoff (max 3 attempts)

### Credential Security

- **macOS**: Stored in macOS Keychain using `security` command
- **Linux**: Stored in Secret Service (GNOME Keyring, KWallet, etc.) using `libsecret`
- **Windows**: Stored in Windows Credential Manager
- **No plaintext**: Credentials never stored in configuration files
- **OS-level encryption**: Leverages OS native secure storage

## Uninstallation

To completely remove the Auto Captive Portal service:

```bash
curl -fsSL https://raw.githubusercontent.com/amansikarwar/auto-captive-portal/main/install.sh | bash -s uninstall
```

This will:

1. Stop and remove the background service
2. Delete stored credentials from keychain
3. Remove the `acp` binary from `/usr/local/bin/`
4. Clean up all service configuration files

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| **"Keyring error"** or **"Credentials not found"** | Run `acp setup` to store credentials |
| **Service not running** | Check logs (see platform-specific commands above) |
| **Login failed** | Run `acp update-credentials` to update credentials |
| **Portal URL/magic extraction fails** | Portal format may have changed - check `captive_portal.rs` |
| **No notifications** | Check notification permissions for the application |

### Diagnostic Commands

```bash
# Check service health and connectivity
acp health

# View detailed service status
acp status

# Check logs
# macOS:
log show --predicate 'processImagePath contains "acp"' --last 5m
# or check file log:
cat ~/.local/share/acp/logs/acp.log

# Linux:
journalctl --user -u acp -n 50
# or check file log:
cat ~/.local/share/acp/logs/acp.log

# Windows (PowerShell):
Get-Content "$env:APPDATA\acp\logs\acp.log" -Tail 50
```

### Manual Testing

To test the service without installing:

```bash
# Build from source
cargo build --release

# Run setup (stores credentials)
./target/release/acp-script setup

# Run daemon directly
RUST_LOG=INFO ./target/release/acp-script

# Or run health check
./target/release/acp-script health
```

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/amansikarwar/auto-captive-portal.git
cd auto-captive-portal

# Build release binary
cargo build --release

# Binary will be at: target/release/acp-script
```

### Cross-Compilation for Release

The project uses GitHub Actions for cross-platform builds:

```yaml
# Targets:
- x86_64-unknown-linux-gnu    → acp-script-linux-amd64
- x86_64-apple-darwin         → acp-script-macos-x86_64
- aarch64-apple-darwin        → acp-script-macos-arm64
- x86_64-pc-windows-msvc      → acp-script-windows-amd64.exe
```

### Linux Build Requirements

For notification support on Linux:

```bash
# Debian/Ubuntu
sudo apt-get install libgtk-3-dev libayatana-appindicator3-dev

# Fedora/RHEL
sudo dnf install gtk3-devel libappindicator-gtk3-devel
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Network monitoring via [netwatcher](https://github.com/mullvad/netwatch)
- Secure credential storage via [keyring-rs](https://github.com/hwchen/keyring-rs)
- Desktop notifications via [notify-rust](https://github.com/hoodie/notify-rust)
