use crate::error::Result;
use log::LevelFilter;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

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
        let log_dir = home_dir
            .join(".local")
            .join("share")
            .join("acp")
            .join("logs");
        fs::create_dir_all(&log_dir)?;
        Ok(log_dir.join("acp.log"))
    }
}

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
        .level_for("reqwest", LevelFilter::Warn)
        .level_for("hyper", LevelFilter::Warn)
        .level_for("rustls", LevelFilter::Warn);

    if !is_service {
        dispatch = dispatch.chain(std::io::stdout());
    }

    match get_log_file_path() {
        Ok(log_path) => match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(log_file) => {
                dispatch = dispatch.chain(log_file);
            }
            Err(e) => {
                eprintln!("Warning: Failed to open log file {:?}: {}", log_path, e);
            }
        },
        Err(e) => {
            eprintln!("Warning: Failed to get log file path: {}", e);
        }
    }

    #[cfg(target_os = "windows")]
    if is_service {}

    dispatch.apply().map_err(|e| {
        crate::error::AppError::Service(format!("Failed to initialize logging: {}", e))
    })?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn register_event_log() -> Result<()> {
    eventlog::register("AutoCaptivePortal").map_err(|e| {
        crate::error::AppError::Service(format!("Failed to register event log: {}", e))
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn deregister_event_log() -> Result<()> {
    eventlog::deregister("AutoCaptivePortal").map_err(|e| {
        crate::error::AppError::Service(format!("Failed to deregister event log: {}", e))
    })?;
    Ok(())
}
