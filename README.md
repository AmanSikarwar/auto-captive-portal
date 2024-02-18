# Auto Captive Portal Login

This project is a Rust application that runs as a continuous systemd service. It checks for IIT Mandi network captive portal every 10 seconds, logs in through the captive portal if detected, and sends a notification through 'notify-send' commands. If any error occurs, the service restarts.

## Prerequisites

- Rust
- Cargo
- Systemd
- notify-send
- Google Chrome

## Setup

Clone the repository:

```bash
git clone https://amansikarwar/auto-captive-portal.git
cd auto-captive-portal
```

(*Optional*, required for running with cargo) Create a `.env` file in the project root directory and provide the following environment variables (replace `<ldap-username>` and `<ldap-password>` with your LDAP credentials

```bash
LDAP_USERNAME=<ldap-username>
LDAP_PASSWORD=<ldap-password>
```

Build the project:

```bash
cargo build --release
```

Create a systemd service file at `~/.config/systemd/user/acp.service`. Provide ***Username*** and ***Password*** when prompted:

```bash
chmod +x create_service.sh
./create_service.sh
```

Start the service:

```bash
systemctl --user start acp
```

Enable the service to start on boot:

```bash
systemctl --user enable acp
```

To check the status of the service

```bash
systemctl --user status acp
```

To see the logs

```bash
journalctl --user -u acp
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
