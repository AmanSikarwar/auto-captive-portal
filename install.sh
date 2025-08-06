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
        local plist_path="$HOME/Library/LaunchAgents/$service_name.plist"
        
        # Check if service is loaded and unload it
        if launchctl list | grep -q "$service_name"; then
            echo "    Unloading service..."
            if ! launchctl unload "$plist_path" 2>/dev/null; then
                echo "    Warning: Failed to unload service (it may not be running)"
            fi
        fi
        
        # Remove plist file
        if [[ -f "$plist_path" ]]; then
            echo "    Removing plist file..."
            rm -f "$plist_path"
        else
            echo "    Plist file not found (may already be removed)"
        fi
        
    elif [[ "$os_type" == "Linux" ]]; then  # Linux (systemd)
        echo "  Stopping and disabling systemd service..."
        
        # Check if systemctl is available
        if ! command -v systemctl &> /dev/null; then
            echo "    Warning: systemctl not found, skipping service removal"
        else
            # Stop the service
            if systemctl --user is-active --quiet "$service_name" 2>/dev/null; then
                echo "    Stopping service..."
                systemctl --user stop "$service_name" 2>/dev/null || echo "    Warning: Failed to stop service"
            fi
            
            # Disable the service
            if systemctl --user is-enabled --quiet "$service_name" 2>/dev/null; then
                echo "    Disabling service..."
                systemctl --user disable "$service_name" 2>/dev/null || echo "    Warning: Failed to disable service"
            fi
            
            # Remove service file
            local service_file="$HOME/.config/systemd/user/$service_name.service"
            if [[ -f "$service_file" ]]; then
                echo "    Removing service file..."
                rm -f "$service_file"
            fi
            
            # Reload daemon
            echo "    Reloading systemd daemon..."
            systemctl --user daemon-reload 2>/dev/null || echo "    Warning: Failed to reload systemd daemon"
        fi
    else
        echo "Error: Unsupported platform for uninstallation." >&2
        return 1
    fi

    echo "  Removing credentials from keychain/secrets..."
    if [[ "$os_type" == "Darwin" ]]; then
        # macOS keychain
        security delete-generic-password -s "$service_name" -a "ldap_username" 2>/dev/null || echo "    Warning: Username not found in keychain"
        security delete-generic-password -s "$service_name" -a "ldap_password" 2>/dev/null || echo "    Warning: Password not found in keychain"
    elif [[ "$os_type" == "Linux" ]]; then
        # Linux secret service
        if command -v secret-tool &> /dev/null; then
            secret-tool clear service "$service_name" username "ldap_username" 2>/dev/null || echo "    Warning: Username not found in secret service"
            secret-tool clear service "$service_name" username "ldap_password" 2>/dev/null || echo "    Warning: Password not found in secret service"
        else
            echo "    Warning: secret-tool not found, cannot clear credentials"
        fi
    fi

    echo "  Removing binary from /usr/local/bin..."
    if [[ -f "/usr/local/bin/acp" ]]; then
        if sudo rm -f /usr/local/bin/acp 2>/dev/null; then
            echo "    Binary removed successfully"
        else
            echo "    Warning: Failed to remove binary (may require manual removal)"
        fi
    else
        echo "    Binary not found (may already be removed)"
    fi

    echo "Auto Captive Portal Service uninstalled successfully."
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
    if ! sudo mv /tmp/acp /usr/local/bin/acp; then
        echo "Error: Failed to move binary to /usr/local/bin. Please ensure you have sudo privileges." >&2
        return 1
    fi
    sudo chmod 755 /usr/local/bin/acp
    echo "  Binary installed to /usr/local/bin/acp"

    echo "  Running setup (storing credentials in keychain)..."
    if ! /usr/local/bin/acp setup; then
        echo "Error: Setup process failed. Please check the output above for errors." >&2
        return 1
    fi
    echo "  Credentials stored successfully."

    # After setup, the service should already be created and started by the Rust application
    # Let's verify the service is running
    echo "  Verifying service status..."
    
    if [[ "$os_type" == "Darwin" ]]; then
        if launchctl list | grep -q "$service_name"; then
            echo "  Service is running successfully."
        else
            echo "  Warning: Service may not be running. Check logs for details."
        fi
    elif [[ "$os_type" == "Linux" ]]; then
        if command -v systemctl &> /dev/null; then
            if systemctl --user is-active --quiet "$service_name" 2>/dev/null; then
                echo "  Service is running successfully."
            else
                echo "  Warning: Service may not be running. Check logs for details."
            fi
        else
            echo "  Warning: Cannot verify service status - systemctl not available."
        fi
    fi

    echo "Auto Captive Portal Service installed!"
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
    echo "Error: jq is required. Please install it:" >&2
    echo "  - On macOS: brew install jq" >&2
    echo "  - On Linux: apt install jq (Debian/Ubuntu) or yum install jq (CentOS/RHEL)" >&2
    exit 1
fi

# Check platform-specific requirements
case "$(uname -s)" in
    "Darwin")
        # Check if we have access to security command for keychain
        if ! command -v security &> /dev/null; then
            echo "Error: security command not found. This is required for macOS keychain access." >&2
            exit 1
        fi
        ;;
    "Linux")
        # On Linux, we should have either secret-tool or gnome-keyring available
        if ! command -v secret-tool &> /dev/null && ! command -v gnome-keyring &> /dev/null; then
            echo "Warning: Neither secret-tool nor gnome-keyring found. Credential storage may not work properly." >&2
            echo "  Install with: apt install libsecret-tools (Debian/Ubuntu)" >&2
        fi
        ;;
esac

# Early sudo check (but don't exit if it fails, as user might provide password later)
if ! sudo -n true 2>/dev/null; then
    echo "Note: This script will require sudo privileges for binary installation."
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