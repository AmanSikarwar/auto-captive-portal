# Auto Captive Portal Login

This Rust application automates authentication for the IIT Mandi captive portal. It runs as a background service, checking for captive portals every 10 seconds, logging in automatically when detected, and sending desktop notifications upon successful login.

## Prerequisites

- **macOS** or **Linux**
- **jq** (required for the installation script):
  - On macOS: `brew install jq`
  - On Linux: `apt install jq` (Debian/Ubuntu) or `yum install jq` (CentOS/RHEL)
- **Rust and Cargo** (only if building from source)

## Installation

To install and set up the Auto Captive Portal Login service, run

```bash
curl -fsSL https://raw.githubusercontent.com/amansikarwar/auto-captive-portal/main/install.sh | bash
```

This command will:

- Download the latest `acp` binary for your platform from GitHub releases.
- Install it to `/usr/local/bin/acp`.
- Run the `setup` command, prompting you for your LDAP credentials to configure the service.

**Note**: You will be prompted for your LDAP credentials during the setup process

### Supported Platforms

- **Linux (x86_64)**
- **macOS (x86_64)**
- **macOS (arm64)**

## Platform-specific Details

### macOS

The service runs as a LaunchAgent and starts automatically on login.

To manually manage the service:

```bash
# Start
launchctl load ~/Library/LaunchAgents/com.user.acp.plist

# Stop
launchctl unload ~/Library/LaunchAgents/com.user.acp.plist

# View logs
log show --predicate 'processImagePath contains "acp"'
```

### Linux

The service runs as a systemd user service.

To manually manage the service:

```bash
# Start
systemctl --user start acp

# Stop
systemctl --user stop acp

# View logs
journalctl --user -u acp
```

## Uninstallation

To uninstall the Auto Captive Portal Login service and remove the binary, run:

```bash
curl -fsSL https://raw.githubusercontent.com/amansikarwar/auto-captive-portal/main/install.sh | bash -s uninstall
```

This will:

- Stop and remove the service.
- Delete the stored credentials.
- Remove the `acp` binary from `/usr/local/bin/`.

## Troubleshooting

If you encounter issues during installation or while running the service, try the following:

- **Check Logs**:
  - macOS: `log show --predicate 'processImagePath contains "acp"'`
  - Linux: `journalctl --user -u acp`
- **Ensure Network Connectivity**: The installation script requires internet access to download the binary.
- **Verify Service Status**:
  - macOS: `launchctl list | grep com.user.acp`
  - Linux: `systemctl --user status acp`
- **Re-run Setup**: If credentials are incorrect, run `/usr/local/bin/acp setup` to re-enter them.
- **Check Permissions**: Ensure you have sudo privileges for moving the binary to `/usr/local/bin/`.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
