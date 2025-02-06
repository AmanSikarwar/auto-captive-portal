# Auto Captive Portal Login

This project is a Rust application that automatically handles IIT Mandi captive portal authentication. It runs as a background service that checks for captive portals every 10 seconds, performs automatic login, and sends desktop notifications on successful authentication.

## Prerequisites

- Rust and Cargo
- macOS or Linux

## Installation

1. Clone the repository:

```bash
git clone https://github.com/amansikarwar/auto-captive-portal.git
cd auto-captive-portal
```

2. Build the project:

```bash
cargo build --release
```

3. Run the setup:

```bash
./target/release/acp-script setup
```

This will:

- Prompt for your LDAP credentials
- Store credentials securely in the system keychain
- Create and start the background service

## Platform-specific Details

### macOS

The service runs as a LaunchAgent and will start automatically on login.

To manually manage the service:

```bash
# Start
launchctl load ~/Library/LaunchAgents/com.user.acp.plist

# Stop
launchctl unload ~/Library/LaunchAgents/com.user.acp.plist

# View logs
log show --predicate 'processImagePath contains "acp-script"'
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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
