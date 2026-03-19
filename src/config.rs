use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::info;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub connectivity_check_url: String,
    pub portal_login_url: String,
    pub max_delay_secs: u64,
    pub min_delay_secs: u64,
    pub max_retries: u32,
    pub initial_retry_delay_secs: u64,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            connectivity_check_url: "http://clients3.google.com/generate_204".to_string(),
            portal_login_url: String::new(),
            max_delay_secs: 1800,
            min_delay_secs: 10,
            max_retries: 3,
            initial_retry_delay_secs: 2,
            log_level: "INFO".to_string(),
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"));
        Ok(app_data.join("acp"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home_dir =
            dirs::home_dir().ok_or_else(|| AppError::Service("Home directory not found".into()))?;
        Ok(home_dir.join(".config").join("acp"))
    }
}

pub fn get_config_file_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

pub fn load_config() -> Result<AppConfig> {
    let config_path = get_config_file_path()?;

    if config_path.exists() {
        let contents = fs::read_to_string(&config_path)?;
        let config: AppConfig = toml::from_str(&contents).map_err(|e| {
            AppError::Service(format!(
                "Failed to parse config file {:?}: {}",
                config_path, e
            ))
        })?;
        info!("Loaded configuration from {}", config_path.display());
        Ok(config)
    } else {
        info!(
            "No config file found at {}. Using defaults.",
            config_path.display()
        );
        Ok(AppConfig::default())
    }
}

pub fn init_config() -> Result<()> {
    let config = load_config()?;

    let config = if let Ok(url) = std::env::var("ACP_CONNECTIVITY_URL") {
        AppConfig {
            connectivity_check_url: url,
            ..config
        }
    } else {
        config
    };

    CONFIG
        .set(config)
        .map_err(|_| AppError::Service("Config already initialized".into()))?;
    Ok(())
}

pub fn get_config() -> &'static AppConfig {
    CONFIG
        .get()
        .expect("Config not initialized. Call init_config() first.")
}

pub fn write_default_config() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("config.toml");
    let default_config = AppConfig::default();

    let contents = toml::to_string_pretty(&default_config)
        .map_err(|e| AppError::Service(format!("Failed to serialize default config: {}", e)))?;

    let header = r#"# Auto Captive Portal (ACP) Configuration
# This file is optional. All values shown below are defaults.
# Edit any value to customize ACP behavior.

"#;

    fs::write(&config_path, format!("{}{}", header, contents))?;
    Ok(config_path)
}
