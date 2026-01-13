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
            .join(format!("{SERVICE_NAME}.plist"));

        if let Some(parent) = plist_path.parent() {
            fs::create_dir_all(parent)?;
        }

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
        <string>run</string>
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
    pub fn create_service(&self) -> Result<()> {
        let service_name = SERVICE_NAME;
        let service_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Service("Home directory not found".into()))?
            .join(".config/systemd/user");

        fs::create_dir_all(&service_dir)?;

        let service_content = format!(
            r#"[Unit]
Description=Auto Captive Portal Login Service

[Service]
Environment=RUST_LOG=INFO
ExecStart={} run
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target"#,
            self.executable_path.display()
        );

        let service_path = service_dir.join(format!("{}.service", service_name));
        fs::write(&service_path, service_content)?;

        info!("Creating systemd service at {}", service_path.display());

        // Reload daemon
        let output = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to reload systemd daemon: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        // Enable service
        let output = std::process::Command::new("systemctl")
            .args(["--user", "enable", service_name])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to enable service: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        // Start service
        let output = std::process::Command::new("systemctl")
            .args(["--user", "start", service_name])
            .output()?;

        if !output.status.success() {
            error!(
                "Failed to start service: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(AppError::Service(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        info!("Service created, enabled, and started successfully.");
        Ok(())
    }

    /// Windows service creation with user account support
    #[cfg(target_os = "windows")]
    pub fn create_service(&self) -> Result<()> {
        self.create_service_with_account(None, None)
    }

    /// Windows service creation with optional user account
    #[cfg(target_os = "windows")]
    pub fn create_service_with_account(
        &self,
        account_name: Option<&str>,
        account_password: Option<&str>,
    ) -> Result<()> {
        use std::ffi::OsString;
        use windows_service::{
            service::{
                ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
            },
            service_manager::{ServiceManager as WinServiceManager, ServiceManagerAccess},
        };

        info!("Installing Windows service...");

        // Register event log source
        if let Err(e) = crate::logging::register_event_log() {
            error!("Warning: Failed to register event log: {}", e);
        }

        let manager = WinServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )
        .map_err(|e| {
            AppError::Service(format!(
                "Failed to connect to Service Control Manager: {}",
                e
            ))
        })?;

        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from("Auto Captive Portal"),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: self.executable_path.clone(),
            launch_arguments: vec![OsString::from("run")],
            dependencies: vec![],
            account_name: account_name.map(OsString::from),
            account_password: account_password.map(OsString::from),
        };

        let service = manager
            .create_service(
                &service_info,
                ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
            )
            .map_err(|e| AppError::Service(format!("Failed to create service: {}", e)))?;

        // Set service description
        service
            .set_description("Automatic captive portal login service for IIT Mandi network")
            .map_err(|e| AppError::Service(format!("Failed to set service description: {}", e)))?;

        // Start the service
        service
            .start::<OsString>(&[])
            .map_err(|e| AppError::Service(format!("Failed to start service: {}", e)))?;

        info!("Windows service installed and started successfully.");
        Ok(())
    }
}

#[allow(dead_code)]
pub async fn restart_service() -> Result<()> {
    info!("Restarting service: {SERVICE_NAME}");

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
        let uid = std::process::Command::new("id")
            .args(["-u"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "501".to_string());

        let service_target = format!("gui/{}/{}", uid, SERVICE_NAME);
        let output: std::process::Output = std::process::Command::new("launchctl")
            .args(["kickstart", "-k", &service_target])
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

    #[cfg(target_os = "windows")]
    {
        windows_service_control::stop_service()?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        windows_service_control::start_service()?;
    }

    info!("Service restarted successfully.");
    Ok(())
}

#[cfg(target_os = "windows")]
pub mod windows_service_control {
    use super::*;
    use std::ffi::OsString;
    use std::time::Duration;
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager as WinServiceManager, ServiceManagerAccess},
    };

    pub fn start_service() -> Result<()> {
        let manager =
            WinServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
                .map_err(|e| AppError::Service(format!("Failed to connect to SCM: {}", e)))?;

        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::START)
            .map_err(|e| AppError::Service(format!("Failed to open service: {}", e)))?;

        service
            .start::<OsString>(&[])
            .map_err(|e| AppError::Service(format!("Failed to start service: {}", e)))?;

        info!("Service started successfully.");
        Ok(())
    }

    pub fn stop_service() -> Result<()> {
        let manager =
            WinServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
                .map_err(|e| AppError::Service(format!("Failed to connect to SCM: {}", e)))?;

        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::STOP)
            .map_err(|e| AppError::Service(format!("Failed to open service: {}", e)))?;

        service
            .stop()
            .map_err(|e| AppError::Service(format!("Failed to stop service: {}", e)))?;

        // Wait for service to stop
        std::thread::sleep(Duration::from_secs(2));

        info!("Service stopped successfully.");
        Ok(())
    }

    pub fn uninstall_service() -> Result<()> {
        let manager =
            WinServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
                .map_err(|e| AppError::Service(format!("Failed to connect to SCM: {}", e)))?;

        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::DELETE)
            .map_err(|e| AppError::Service(format!("Failed to open service: {}", e)))?;

        let _ = service.stop();
        std::thread::sleep(Duration::from_secs(2));

        service
            .delete()
            .map_err(|e| AppError::Service(format!("Failed to delete service: {}", e)))?;

        if let Err(e) = crate::logging::deregister_event_log() {
            error!("Warning: Failed to deregister event log: {}", e);
        }

        info!("Service uninstalled successfully.");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub mod windows_service_main {
    use log::info;
    use std::ffi::OsString;
    use std::sync::mpsc;
    use std::time::Duration;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    define_windows_service!(ffi_service_main, service_main);

    pub fn run_as_windows_service() -> windows_service::Result<()> {
        service_dispatcher::start(super::SERVICE_NAME, ffi_service_main)
    }

    fn service_main(_arguments: Vec<OsString>) {
        if let Err(e) = run_service() {
            log::error!("Service error: {}", e);
        }
    }

    fn run_service() -> windows_service::Result<()> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    info!("Received stop signal from Windows Service Manager");
                    shutdown_tx.send(()).ok();
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(super::SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        info!("Windows service started and running");

        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async { run_service_loop(shutdown_rx).await });

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        info!("Windows service stopped");
        Ok(())
    }

    async fn run_service_loop(shutdown_rx: mpsc::Receiver<()>) {
        use crate::captive_portal;
        use crate::error::AppError;
        use crate::notifications;
        use keyring::Entry;
        use log::{error, info};
        use tokio::sync::mpsc as tokio_mpsc;

        let (username, password) = match get_credentials_internal() {
            Ok(creds) => creds,
            Err(e) => {
                error!("Failed to get credentials: {}", e);
                return;
            }
        };

        const MAX_DELAY_SECS: u64 = 1800;
        const MIN_DELAY_SECS: u64 = 10;
        const CHANNEL_CAPACITY: usize = 10;
        let mut sleep_duration = Duration::from_secs(MIN_DELAY_SECS);

        // Use bounded channel to prevent unbounded memory growth
        let (tx, mut rx) = tokio_mpsc::channel(CHANNEL_CAPACITY);

        let _watcher_handle = match netwatcher::watch_interfaces(move |update| {
            if update.diff.added.is_empty()
                && update.diff.removed.is_empty()
                && update.diff.modified.is_empty()
            {
                return;
            }

            let has_relevant_change = !update.diff.added.is_empty()
                || update
                    .diff
                    .modified
                    .values()
                    .any(|d| !d.addrs_added.is_empty());

            if has_relevant_change {
                info!("Network change detected");
                let _ = tx.try_send(());
            }
        }) {
            Ok(handle) => handle,
            Err(e) => {
                error!("Failed to start network watcher: {}", e);
                return;
            }
        };

        info!("Performing initial captive portal check...");
        if let Ok(true) = check_and_login_internal(&username, &password).await {
            sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
        }

        loop {
            if shutdown_rx.try_recv().is_ok() {
                info!("Shutdown signal received, exiting service loop");
                break;
            }

            tokio::select! {
                biased;

                Some(_) = rx.recv() => {
                    info!("Network change signal received");
                    tokio::time::sleep(Duration::from_secs(3)).await;

                    if let Ok(true) = check_and_login_internal(&username, &password).await {
                        sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                    } else {
                        sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                    }
                },

                _ = tokio::time::sleep(sleep_duration) => {
                    info!("Polling interval elapsed");
                    match check_and_login_internal(&username, &password).await {
                        Ok(true) => {
                            sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                        },
                        Ok(false) => {
                            let current_secs = sleep_duration.as_secs();
                            let next_secs = (current_secs / 2).max(MIN_DELAY_SECS);
                            sleep_duration = Duration::from_secs(next_secs);
                        },
                        Err(_) => {
                            sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                        }
                    }
                },
            }
        }
    }

    fn get_credentials_internal() -> crate::error::Result<(String, String)> {
        let username_entry = Entry::new(super::SERVICE_NAME, "ldap_username")
            .map_err(crate::error::AppError::from)?;
        let password_entry = Entry::new(super::SERVICE_NAME, "ldap_password")
            .map_err(crate::error::AppError::from)?;
        Ok((
            username_entry
                .get_password()
                .map_err(crate::error::AppError::from)?,
            password_entry
                .get_password()
                .map_err(crate::error::AppError::from)?,
        ))
    }

    async fn check_and_login_internal(
        username: &str,
        password: &str,
    ) -> crate::error::Result<bool> {
        use crate::captive_portal;
        use crate::notifications;
        use log::{error, info};

        match captive_portal::check_captive_portal().await {
            Ok(Some((url, magic))) => {
                info!("Captive portal detected at {}", url);
                match captive_portal::login_with_retry(&url, username, password, &magic).await {
                    Ok(_) => {
                        notifications::send_notification(
                            "Logged into captive portal successfully.",
                        )
                        .await;
                        info!("Login successful");
                        Ok(true)
                    }
                    Err(e) => {
                        error!("Login failed: {}", e);
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                info!("No captive portal detected");
                Ok(false)
            }
            Err(e) => {
                error!("Portal check failed: {}", e);
                Err(e)
            }
        }
    }
}
