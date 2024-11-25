use crate::error::{AppError, Result};
use keyring::Entry;
use log::{error, info};
use std::fs;
use std::path::PathBuf;

pub const SERVICE_NAME: &str = if cfg!(target_os = "macos") {
    "com.user.acp"
} else {
    "acp"
};

pub struct ServiceManager {
    executable_path: PathBuf,
}

impl ServiceManager {
    pub fn new(executable_path: PathBuf) -> Self {
        Self { executable_path }
    }

    pub fn store_credentials(&self, username: &str, password: &str) -> Result<()> {
        let username_entry: Entry = Entry::new(SERVICE_NAME, "ldap_username")?;
        username_entry.set_password(username)?;

        let password_entry: Entry = Entry::new(SERVICE_NAME, "ldap_password")?;
        password_entry.set_password(password)?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn create_service(&self) -> Result<()> {
        let plist_path: PathBuf = dirs::home_dir()
            .ok_or_else(|| AppError::Service("Home directory not found".into()))?
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", SERVICE_NAME));

        fs::create_dir_all(plist_path.parent().unwrap())?;

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>INFO</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>"#,
            SERVICE_NAME,
            self.executable_path.display()
        );

        info!("Creating service at {}", plist_path.display());

        fs::write(&plist_path, plist_content)?;

        let output: std::process::Output = std::process::Command::new("launchctl")
            .args(["load", plist_path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to load service: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        info!("Service created and loaded successfully.");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn create_service(&self) -> io::Result<()> {
        let service_name = SERVICE_NAME;
        let service_dir = dirs::home_dir().unwrap().join(".config/systemd/user");

        fs::create_dir_all(&service_dir)?;

        let service_content = format!(
            r#"[Unit]
Description=Auto Captive Portal Login Service

[Service]
Environment=RUST_LOG=INFO
ExecStart={}
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target"#,
            self.executable_path.display()
        );

        let service_path = service_dir.join(format!("{}.service", service_name));
        fs::write(&service_path, service_content)?;

        std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;

        std::process::Command::new("systemctl")
            .args(["--user", "enable", service_name])
            .output()?;

        std::process::Command::new("systemctl")
            .args(["--user", "start", service_name])
            .output()?;

        Ok(())
    }
}

pub async fn restart_service() -> Result<()> {
    info!("Restarting service: {}", SERVICE_NAME);

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("systemctl")
            .args(["--user", "restart", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to restart service: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output: std::process::Output = std::process::Command::new("launchctl")
            .args(["kickstart", "-k", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to restart service: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
    }

    info!("Service restarted successfully.");
    Ok(())
}
