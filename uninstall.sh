#!/bin/bash

# --- Script Name: uninstall.sh ---
# --- Description: Uninstalls acp, an Auto Captive Portal login service. ---

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

# --- Main execution ---
echo "This will completely remove the Auto Captive Portal Service from your system."
read -p "Are you sure you want to continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Uninstallation cancelled."
    exit 0
fi

uninstall_service

if [ $? -eq 0 ]; then
    echo "Uninstallation completed successfully!"
else
    echo "Uninstallation failed. Please check the error messages above." >&2
    exit 1
fi

exit 0
