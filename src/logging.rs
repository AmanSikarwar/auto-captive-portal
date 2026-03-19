use crate::error::Result;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

const MAX_LOG_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB
const MAX_LOG_FILES: usize = 3;

fn rotate_logs_if_needed(log_path: &PathBuf) {
    if let Ok(metadata) = fs::metadata(log_path)
        && metadata.len() >= MAX_LOG_SIZE_BYTES
    {
        let oldest_path = log_path.with_extension(format!("log.{}", MAX_LOG_FILES));
        if oldest_path.exists()
            && let Err(e) = fs::remove_file(&oldest_path)
        {
            eprintln!(
                "Warning: Failed to remove oldest log archive {:?}: {}",
                oldest_path, e
            );
        }

        for i in (1..MAX_LOG_FILES).rev() {
            let old_path = log_path.with_extension(format!("log.{}", i));
            let new_path = log_path.with_extension(format!("log.{}", i + 1));
            if old_path.exists() {
                if new_path.exists()
                    && let Err(e) = fs::remove_file(&new_path)
                {
                    eprintln!(
                        "Warning: Failed to remove {:?} before rotation: {}",
                        new_path, e
                    );
                    continue;
                }
                if let Err(e) = fs::rename(&old_path, &new_path) {
                    eprintln!(
                        "Warning: Failed to rotate log {:?} -> {:?}: {}",
                        old_path, new_path, e
                    );
                }
            }
        }

        let rotated_path = log_path.with_extension("log.1");
        if rotated_path.exists()
            && let Err(e) = fs::remove_file(&rotated_path)
        {
            eprintln!(
                "Warning: Failed to remove {:?} before rotation: {}",
                rotated_path, e
            );
            return;
        }
        if let Err(e) = fs::rename(log_path, &rotated_path) {
            eprintln!(
                "Warning: Failed to rotate current log {:?} -> {:?}: {}",
                log_path, rotated_path, e
            );
        }
    }
}

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
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info")
            .add_directive("reqwest=warn".parse().unwrap())
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("rustls=warn".parse().unwrap())
            .add_directive("html5ever=warn".parse().unwrap())
            .add_directive("selectors=warn".parse().unwrap())
    });

    match get_log_file_path() {
        Ok(log_path) => {
            rotate_logs_if_needed(&log_path);

            match OpenOptions::new().create(true).append(true).open(&log_path) {
                Ok(log_file) => {
                    if is_service {
                        tracing_subscriber::fmt()
                            .with_env_filter(env_filter)
                            .with_writer(log_file)
                            .with_ansi(false)
                            .init();
                    } else {
                        use tracing_subscriber::fmt::writer::MakeWriterExt;
                        let file_writer = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_path)
                            .expect("Failed to open log file for writer");
                        let writer = std::io::stdout.and(file_writer);
                        tracing_subscriber::fmt()
                            .with_env_filter(env_filter)
                            .with_writer(writer)
                            .with_ansi(false)
                            .init();
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to open log file {:?}: {}", log_path, e);
                    tracing_subscriber::fmt().with_env_filter(env_filter).init();
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to get log file path: {}", e);
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
        }
    }

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
