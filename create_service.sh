#!/bin/bash

SERVICE_NAME=acp
SERVICE_DESCRIPTION="Auto Captive Portal Login Service"
EXECUTABLE_PATH="$PWD/target/release/acp-script"

# Prompt the user for LDAP credentials
read -p "Enter LDAP Username: " LDAP_USERNAME
read -sp "Enter LDAP Password: " LDAP_PASSWORD
echo

cat > ~/.config/systemd/user/${SERVICE_NAME}.service << EOF
[Unit]
Description=${SERVICE_DESCRIPTION}
# Requires=network.target
# After=network.target

[Service]
ExecStart=${EXECUTABLE_PATH}
Environment="LDAP_USERNAME=${LDAP_USERNAME}"
Environment="LDAP_PASSWORD=${LDAP_PASSWORD}"
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
EOF

echo "Service file created for ${SERVICE_NAME}"