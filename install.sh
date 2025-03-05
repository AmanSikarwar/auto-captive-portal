#!/bin/bash

# --- Script Name: install.sh ---
# --- Description: Installs acp, an Auto Captive Portal login service. ---

# --- Function to get the latest release tag from GitHub ---
get_latest_tag() {
    curl -sL --fail "https://api.github.com/repos/amansikarwar/auto-captive-portal/releases/latest" | jq -r '.tag_name'
}

# --- Function to download the correct binary for the platform ---
download_binary() {
    local tag="$1"
    local asset_name=""
    local temp_file="/tmp/acp"

    case "$(uname -sm)" in
        "Darwin x86_64") asset_name="acp-script-macos-x86_64" ;;
        "Darwin arm64") asset_name="acp-script-macos-arm64" ;;
        "Linux x86_64") asset_name="acp-script-linux-amd64" ;;
        *)
            echo "Error: Unsupported platform: $(uname -sm). Supported platforms are: Darwin x86_64, Darwin arm64, Linux x86_64." >&2
            exit 1
            ;;
    esac

    echo "Downloading release: $tag"
    curl -sL --fail -o "$temp_file" "https://github.com/amansikarwar/auto-captive-portal/releases/download/${tag}/${asset_name}"
    if [ $? -ne 0 ]; then
        echo "Error: Failed to download the binary. Please check your network connection or the asset name." >&2
        rm -f "$temp_file"
        exit 1
    fi
    chmod +x "$temp_file"
}

# --- Function to uninstall the service ---
uninstall_service() {
    local os_type=$(uname -s)
    local service_name="acp"  # Default for Linux
    if [[ "$os_type" == "Darwin" ]]; then
        service_name="com.user.acp"
    fi

    echo "Uninstalling Auto Captive Portal Service..."

    if [[ "$os_type" == "Darwin" ]]; then  # macOS
        echo "  Stopping and unloading LaunchAgent..."
        if launchctl list | grep -q "$service_name"; then
            launchctl bootout gui/"$(id -u)"/"$service_name" 2>/dev/null || true
        fi
        rm -f "$HOME/Library/LaunchAgents/$service_name.plist"
        launchctl unload "$HOME/Library/LaunchAgents/$service_name.plist" 2>/dev/null || true
    elif [[ "$os_type" == "Linux" ]]; then  # Linux (systemd)
        echo "  Stopping and disabling systemd service..."
        systemctl --user stop "$service_name" 2>/dev/null || true
        systemctl --user disable "$service_name" 2>/dev/null || true
        rm -f "$HOME/.config/systemd/user/$service_name.service"
        systemctl --user daemon-reload 2>/dev/null || true
    else
        echo "Error: Unsupported platform for uninstallation." >&2
        return 1
    fi

    echo "  Removing credentials from keychain/secrets..."
    security delete-generic-password -s "$service_name" -a "ldap_username" 2>/dev/null || true # macOS
    security delete-generic-password -s "$service_name" -a "ldap_password" 2>/dev/null || true # macOS
    secret-tool clear --quiet service="$service_name" login="ldap_username" 2>/dev/null || true # Linux
    secret-tool clear --quiet service="$service_name" login="ldap_password" 2>/dev/null || true # Linux

    echo "  Removing binary from /usr/local/bin..."
    sudo rm -f /usr/local/bin/acp 2>/dev/null || true

    echo "Auto Captive Portal Service uninstalled."
    return 0
}

# --- Function to install the service ---
install_service() {
    local os_type=$(uname -s)
    local service_name="acp"
    if [[ "$os_type" == "Darwin" ]]; then
        service_name="com.user.acp"
    fi

    echo "Installing Auto Captive Portal Service..."

    echo "  Moving binary to /usr/local/bin..."
    sudo mv /tmp/acp /usr/local/bin/acp
    if [ $? -ne 0 ]; then
        echo "Error: Failed to move binary to /usr/local/bin. Please ensure you have sudo privileges." >&2
        return 1
    fi
    sudo chmod 755 /usr/local/bin/acp
    echo "  Binary installed to /usr/local/bin/acp"

    echo "  Running setup (storing credentials in keychain)..."
    /usr/local/bin/acp setup
    if [ $? -ne 0 ]; then
        echo "Error: Setup process failed. Please check the output above for errors." >&2
        return 1
    fi
    echo "  Credentials stored successfully."

    echo "Auto Captive Portal Service installed and running!"
    return 0
}

# --- Main execution ---
if [[ "$1" == "uninstall" ]]; then
    uninstall_service
    exit
elif [[ "$1" == "install" ]]; then
    # Proceed with installation
    :
else
    # Default action is installation
    :
fi

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required. Please install it (e.g., apt install jq, brew install jq)." >&2
    exit 1
fi

# Check if user has sudo privileges (early check, but not for setup itself)
if ! sudo -n true 2>/dev/null; then
    echo "Error: This script requires sudo for final binary installation and service management. Run with sudo access if prompted." >&2
fi

# Get latest tag with retry mechanism
max_retries=5
retry_delay=5
retry_count=0
latest_tag=""

while [[ $retry_count -lt $max_retries ]]; do
    latest_tag=$(get_latest_tag)
    if [[ -n "$latest_tag" ]]; then
        break
    fi
    echo "Failed to retrieve latest tag. Retrying in $retry_delay seconds..."
    sleep $retry_delay
    ((retry_count++))
done

if [[ -z "$latest_tag" ]]; then
    echo "Error: Could not get latest release tag after $max_retries tries. Please check your network connection." >&2
    exit 1
fi

# Download binary
download_binary "$latest_tag"

# Install the service
install_service

if [ $? -eq 0 ]; then
    echo "Installation completed successfully!"
else
    echo "Installation failed. Please check the error messages above." >&2
    exit 1
fi

exit 0