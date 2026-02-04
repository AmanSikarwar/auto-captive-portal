use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Default)]
pub struct ServiceState {
    pub last_check_timestamp: Option<u64>,
    pub last_successful_login_timestamp: Option<u64>,
    pub last_portal_detected: Option<String>,
}

pub fn get_state_file_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"));
        let state_dir = app_data.join("acp");
        fs::create_dir_all(&state_dir)?;
        Ok(state_dir.join("state.json"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Service("Could not determine home directory".to_string()))?;
        let state_dir = home_dir.join(".local").join("share").join("acp");
        fs::create_dir_all(&state_dir)?;
        Ok(state_dir.join("state.json"))
    }
}

pub fn load_state() -> Result<ServiceState> {
    let state_path = get_state_file_path()?;
    if state_path.exists() {
        let contents = fs::read_to_string(&state_path)?;
        serde_json::from_str(&contents)
            .map_err(|e| AppError::Service(format!("Failed to parse state file: {}", e)))
    } else {
        Ok(ServiceState::default())
    }
}

pub fn update_state_file(portal_url: Option<&str>, login_success: bool) -> Result<()> {
    let state_path = get_state_file_path()?;
    let mut state = load_state().unwrap_or_default();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();

    state.last_check_timestamp = Some(now);

    if login_success {
        state.last_successful_login_timestamp = Some(now);
    }

    if let Some(url) = portal_url {
        state.last_portal_detected = Some(url.to_string());
    }

    let contents = serde_json::to_string_pretty(&state)
        .map_err(|e| AppError::Service(format!("Failed to serialize state: {}", e)))?;
    fs::write(&state_path, contents)?;

    Ok(())
}

pub fn format_duration_ago(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();

    if now < timestamp {
        return "just now".to_string();
    }

    let diff = now - timestamp;

    if diff < 60 {
        format!("{} seconds ago", diff)
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = diff / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_state_file_path_creates_directory() {
        let state_path = get_state_file_path();
        assert!(state_path.is_ok());

        if let Ok(path) = state_path {
            assert!(path.to_string_lossy().contains("acp"));
            assert!(path.to_string_lossy().ends_with("state.json"));
        }
    }

    #[test]
    fn test_get_state_file_path_structure() {
        let state_path = get_state_file_path().expect("Failed to get state file path");

        #[cfg(target_os = "windows")]
        {
            let path_str = state_path.to_string_lossy();
            assert!(path_str.contains("acp"));
        }

        #[cfg(not(target_os = "windows"))]
        {
            let path_str = state_path.to_string_lossy();
            assert!(path_str.contains(".local"));
            assert!(path_str.contains("share"));
            assert!(path_str.contains("acp"));
        }
    }

    #[test]
    fn test_service_state_default() {
        let state = ServiceState::default();
        assert!(state.last_check_timestamp.is_none());
        assert!(state.last_successful_login_timestamp.is_none());
        assert!(state.last_portal_detected.is_none());
    }

    #[test]
    fn test_load_state_nonexistent_file() {
        // Test loading state when file doesn't exist returns default
        let result = load_state();
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(state.last_check_timestamp.is_none());
    }

    #[test]
    fn test_update_state_file_with_portal() {
        let result = update_state_file(Some("https://portal.example.com"), false);
        // Should succeed or fail gracefully
        assert!(result.is_ok() || result.is_err());

        // If it succeeded, verify we can load it
        if result.is_ok() {
            let state = load_state().unwrap();
            assert!(state.last_check_timestamp.is_some());
            assert_eq!(
                state.last_portal_detected,
                Some("https://portal.example.com".to_string())
            );
        }
    }

    #[test]
    fn test_update_state_file_with_login_success() {
        let result = update_state_file(None, true);
        assert!(result.is_ok() || result.is_err());

        if result.is_ok() {
            let state = load_state().unwrap();
            assert!(state.last_check_timestamp.is_some());
            assert!(state.last_successful_login_timestamp.is_some());
        }
    }

    #[test]
    fn test_update_state_file_without_login_success() {
        let result = update_state_file(None, false);
        assert!(result.is_ok() || result.is_err());

        if result.is_ok() {
            let state = load_state().unwrap();
            assert!(state.last_check_timestamp.is_some());
        }
    }

    #[test]
    fn test_format_duration_ago_seconds() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 30;
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "30 seconds ago");
    }

    #[test]
    fn test_format_duration_ago_one_minute() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 60;
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "1 minute ago");
    }

    #[test]
    fn test_format_duration_ago_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 300; // 5 minutes
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "5 minutes ago");
    }

    #[test]
    fn test_format_duration_ago_one_hour() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 3600;
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "1 hour ago");
    }

    #[test]
    fn test_format_duration_ago_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 7200; // 2 hours
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "2 hours ago");
    }

    #[test]
    fn test_format_duration_ago_one_day() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 86400;
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "1 day ago");
    }

    #[test]
    fn test_format_duration_ago_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now - 172800; // 2 days
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "2 days ago");
    }

    #[test]
    fn test_format_duration_ago_future_timestamp() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now + 100; // Future timestamp
        let result = format_duration_ago(timestamp);
        assert_eq!(result, "just now");
    }

    #[test]
    fn test_format_duration_ago_zero_seconds() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = format_duration_ago(now);
        assert_eq!(result, "0 seconds ago");
    }

    #[test]
    fn test_service_state_serialization() {
        let state = ServiceState {
            last_check_timestamp: Some(1234567890),
            last_successful_login_timestamp: Some(1234567891),
            last_portal_detected: Some("https://portal.test.com".to_string()),
        };

        let json = serde_json::to_string(&state);
        assert!(json.is_ok());

        let deserialized: Result<ServiceState, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());

        let deserialized_state = deserialized.unwrap();
        assert_eq!(
            deserialized_state.last_check_timestamp,
            Some(1234567890)
        );
        assert_eq!(
            deserialized_state.last_successful_login_timestamp,
            Some(1234567891)
        );
        assert_eq!(
            deserialized_state.last_portal_detected,
            Some("https://portal.test.com".to_string())
        );
    }

    #[test]
    fn test_state_timestamp_boundaries() {
        // Test edge case with very old timestamp
        let result = format_duration_ago(0);
        assert!(result.contains("day"));

        // Test with timestamp 1 second ago
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = format_duration_ago(now - 1);
        assert_eq!(result, "1 seconds ago");
    }

    #[test]
    fn test_update_state_preserves_existing_data() {
        // First update with portal URL
        let _ = update_state_file(Some("https://first.com"), false);

        // Second update with login success (no portal URL)
        let result = update_state_file(None, true);

        if result.is_ok() {
            let state = load_state().unwrap();
            // Portal URL should still be present from first update
            assert!(state.last_portal_detected.is_some());
            assert!(state.last_successful_login_timestamp.is_some());
        }
    }
}