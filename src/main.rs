mod captive_portal;
mod error;
mod notifications;
mod service;

use console::Term;
use error::{AppError, Result};
use keyring::Entry;
use log::{error, info};
use service::{ServiceManager, SERVICE_NAME};
use std::env;

fn get_credentials() -> Result<(String, String)> {
    let username_entry: Entry =
        Entry::new(SERVICE_NAME, "ldap_username").map_err(AppError::from)?;
    let password_entry: Entry =
        Entry::new(SERVICE_NAME, "ldap_password").map_err(AppError::from)?;
    Ok((
        username_entry.get_password().map_err(AppError::from)?,
        password_entry.get_password().map_err(AppError::from)?,
    ))
}

fn prompt_input(prompt: &str, is_password: bool) -> std::io::Result<String> {
    let term = Term::stdout();
    term.write_str(prompt)?;
    term.flush()?;
    let input = if is_password {
        term.read_secure_line()?
    } else {
        term.read_line()?
    };
    Ok(input.trim().to_string())
}

async fn setup() -> Result<()> {
    info!("Starting setup for Auto Captive Portal...");

    let username: String = prompt_input("Enter LDAP Username: ", false).map_err(AppError::from)?;
    let password: String = prompt_input("Enter LDAP Password: ", true).map_err(AppError::from)?;

    let executable_path: std::path::PathBuf = env::current_exe()?;
    let service_manager: ServiceManager = ServiceManager::new(executable_path);

    service_manager.store_credentials(&username, &password)?;
    service_manager.create_service()?;

    info!("Setup completed successfully.");
    Ok(())
}

async fn run() -> Result<()> {
    let (username, password) = get_credentials()?;

    loop {
        match captive_portal::check_captive_portal().await {
            Ok(Some((url, magic))) => {
                info!("Captive portal detected at {}", url);
                if let Err(e) = captive_portal::login(&url, &username, &password, &magic).await {
                    error!("Login failed: {}", e);
                    service::restart_service().await?;
                } else {
                    notifications::send_notification(
                        "Captive portal detected and logged in successfully",
                    )
                    .await;
                    info!("Logged into captive portal successfully.");
                }
            }
            Ok(None) => info!("No captive portal detected."),
            Err(e) => {
                error!("Portal check failed: {}", e);
                service::restart_service().await?;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    if env::args().nth(1).as_deref() == Some("setup") {
        if let Err(e) = setup().await {
            error!("Setup failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = run().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}
