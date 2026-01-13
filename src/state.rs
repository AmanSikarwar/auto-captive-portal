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

#[allow(dead_code)]
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

    let mut state: ServiceState = if state_path.exists() {
        fs::read_to_string(&state_path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    } else {
        ServiceState::default()
    };

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
