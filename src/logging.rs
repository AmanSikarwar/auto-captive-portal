use crate::error::Result;
use log::LevelFilter;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

/// Get the log file path based on the platform
fn get_log_file_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"));
        let log_dir = app_data.join("acp").join("logs");
        fs::create_dir_all(&log_dir)?;
        Ok(log_dir.join("acp.log"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| crate::error::AppError::Service("Home directory not found".into()))?;
        let log_dir = home_dir.join(".local").join("share").join("acp").join("logs");
        fs::create_dir_all(&log_dir)?;
        Ok(log_dir.join("acp.log"))
    }
}

/// Initialize logging with both console and file output
/// 
/// On Windows services, also registers with Windows Event Log
pub fn init_logging(is_service: bool) -> Result<()> {
    let log_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(LevelFilter::Info);

    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log_level)
        // Filter out noisy dependencies
        .level_for("reqwest", LevelFilter::Warn)
        .level_for("hyper", LevelFilter::Warn)
        .level_for("rustls", LevelFilter::Warn);

    // Add console output only for interactive mode (not when running as service)
    if !is_service {
        dispatch = dispatch.chain(std::io::stdout());
    }

    // Add file logging
    if let Ok(log_path) = get_log_file_path() {
        if let Ok(log_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            dispatch = dispatch.chain(log_file);
        }
    }

    // On Windows, also log to Windows Event Log when running as a service
    #[cfg(target_os = "windows")]
    if is_service {
        // eventlog integration happens separately - we just ensure file logging works
        // The eventlog crate handles its own initialization
    }

    dispatch.apply().map_err(|e| {
        crate::error::AppError::Service(format!("Failed to initialize logging: {}", e))
    })?;

    Ok(())
}

/// Register the application with Windows Event Log (call during install)
#[cfg(target_os = "windows")]
pub fn register_event_log() -> Result<()> {
    eventlog::register("AutoCaptivePortal").map_err(|e| {
        crate::error::AppError::Service(format!("Failed to register event log: {}", e))
    })?;
    Ok(())
}

/// Deregister the application from Windows Event Log (call during uninstall)
#[cfg(target_os = "windows")]
pub fn deregister_event_log() -> Result<()> {
    eventlog::deregister("AutoCaptivePortal").map_err(|e| {
        crate::error::AppError::Service(format!("Failed to deregister event log: {}", e))
    })?;
    Ok(())
}
