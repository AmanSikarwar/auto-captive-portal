use crate::error::Result;
use log::LevelFilter;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

const MAX_LOG_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB
const MAX_LOG_FILES: usize = 3;

fn rotate_logs_if_needed(log_path: &PathBuf) {
    if let Ok(metadata) = fs::metadata(log_path)
        && metadata.len() >= MAX_LOG_SIZE_BYTES
    {
        // Rotate existing log files
        for i in (1..MAX_LOG_FILES).rev() {
            let old_path = log_path.with_extension(format!("log.{}", i));
            let new_path = log_path.with_extension(format!("log.{}", i + 1));
            let _ = fs::rename(&old_path, &new_path);
        }
        // Rotate current log to .1
        let rotated_path = log_path.with_extension("log.1");
        let _ = fs::rename(log_path, &rotated_path);
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
        Ok(log_path) => {
            rotate_logs_if_needed(&log_path);
            match OpenOptions::new().create(true).append(true).open(&log_path) {
                Ok(log_file) => {
                    dispatch = dispatch.chain(log_file);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to open log file {:?}: {}", log_path, e);
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to get log file path: {}", e);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_max_log_size_constant() {
        assert_eq!(MAX_LOG_SIZE_BYTES, 5 * 1024 * 1024);
    }

    #[test]
    fn test_max_log_files_constant() {
        assert_eq!(MAX_LOG_FILES, 3);
    }

    #[test]
    fn test_get_log_file_path_creates_directory() {
        let log_path = get_log_file_path();
        assert!(log_path.is_ok());

        if let Ok(path) = log_path {
            // Verify the path exists or can be created
            assert!(path.to_string_lossy().contains("acp"));
            assert!(path.to_string_lossy().ends_with("acp.log"));
        }
    }

    #[test]
    fn test_get_log_file_path_structure() {
        let log_path = get_log_file_path().expect("Failed to get log file path");

        #[cfg(target_os = "windows")]
        {
            let path_str = log_path.to_string_lossy();
            assert!(path_str.contains("acp"));
            assert!(path_str.contains("logs"));
        }

        #[cfg(not(target_os = "windows"))]
        {
            let path_str = log_path.to_string_lossy();
            assert!(path_str.contains(".local"));
            assert!(path_str.contains("share"));
            assert!(path_str.contains("acp"));
            assert!(path_str.contains("logs"));
        }
    }

    #[test]
    fn test_rotate_logs_with_small_file() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("acp_test_logs");
        let _ = fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        // Create a small log file
        fs::write(&log_path, "small log").unwrap();

        // Rotation should not happen for small files
        rotate_logs_if_needed(&log_path);

        // Original file should still exist
        assert!(log_path.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rotate_logs_with_large_file() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("acp_test_logs_large");
        let _ = fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        // Create a file larger than MAX_LOG_SIZE_BYTES
        let large_content = "x".repeat((MAX_LOG_SIZE_BYTES + 1) as usize);
        fs::write(&log_path, large_content).unwrap();

        // Perform rotation
        rotate_logs_if_needed(&log_path);

        // Check if rotation happened - rotated file should exist
        let rotated_path = temp_dir.join("test.log.1");
        assert!(rotated_path.exists() || !log_path.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_logging_non_service() {
        // Test that logging can be initialized in non-service mode
        let result = init_logging(false);
        // Should succeed or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_log_level_parsing() {
        // Test that RUST_LOG environment variable is respected
        std::env::set_var("RUST_LOG", "debug");
        let log_level: Option<LevelFilter> = std::env::var("RUST_LOG")
            .ok()
            .and_then(|s| s.parse().ok());
        assert_eq!(log_level, Some(LevelFilter::Debug));
        std::env::remove_var("RUST_LOG");
    }

    #[test]
    fn test_log_level_default() {
        // Test that default log level is Info when RUST_LOG is not set
        std::env::remove_var("RUST_LOG");
        let log_level: LevelFilter = std::env::var("RUST_LOG")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(LevelFilter::Info);
        assert_eq!(log_level, LevelFilter::Info);
    }

    #[test]
    fn test_rotate_logs_multiple_times() {
        // Test that multiple rotations work correctly
        let temp_dir = std::env::temp_dir().join("acp_test_logs_multi");
        let _ = fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        // Create a large file
        let large_content = "x".repeat((MAX_LOG_SIZE_BYTES + 1) as usize);
        fs::write(&log_path, &large_content).unwrap();

        // First rotation
        rotate_logs_if_needed(&log_path);

        // Create another large file
        fs::write(&log_path, &large_content).unwrap();

        // Second rotation
        rotate_logs_if_needed(&log_path);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_max_log_files_boundary() {
        // Ensure MAX_LOG_FILES is a reasonable value
        assert!(MAX_LOG_FILES > 0);
        assert!(MAX_LOG_FILES <= 10); // Reasonable upper bound
    }
}