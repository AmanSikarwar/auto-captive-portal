mod captive_portal;
mod error;
mod notifications;
mod service;

use console::Term;
use error::{AppError, Result};
use keyring::Entry;
use log::{error, info};
use service::{SERVICE_NAME, ServiceManager};
use std::env;
use tokio::sync::mpsc;

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

async fn check_and_login(username: &str, password: &str) -> Result<bool> {
    match captive_portal::check_captive_portal().await {
        Ok(Some((url, magic))) => {
            info!("Captive portal detected at {url}");
            match captive_portal::login_with_retry(&url, username, password, &magic).await {
                Ok(_) => {
                    notifications::send_notification("Logged into captive portal successfully.")
                        .await;
                    info!("Logged into captive portal successfully.");
                    Ok(true)
                }
                Err(e) => {
                    error!("Login failed after all retry attempts: {e}");
                    Err(e)
                }
            }
        }
        Ok(None) => {
            info!("No captive portal detected.");
            Ok(false)
        }
        Err(e) => {
            error!("Portal check failed: {e}");
            Err(e)
        }
    }
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

    const MAX_DELAY_SECS: u64 = 1800;
    const MIN_DELAY_SECS: u64 = 10;
    let mut sleep_duration: std::time::Duration = tokio::time::Duration::from_secs(MIN_DELAY_SECS);

    let (tx, mut rx) = mpsc::unbounded_channel();

    let _watcher_handle = netwatcher::watch_interfaces(move |update| {
        if update.diff.added.is_empty()
            && update.diff.removed.is_empty()
            && update.diff.modified.is_empty()
        {
            info!("Watcher initialized with current network state.");
            return;
        }

        let has_relevant_change = !update.diff.added.is_empty()
            || update
                .diff
                .modified
                .values()
                .any(|d| !d.addrs_added.is_empty());

        if has_relevant_change {
            info!("Relevant network change detected: a new interface or IP address was added.");
            if let Err(e) = tx.send(()) {
                error!("Failed to send network change signal: {e}");
            }
        } else {
            info!("Ignoring irrelevant network change (e.g., interface or IP removed).");
        }
    })
    .map_err(|e| AppError::Service(e.to_string()))?;

    info!("Performing initial check for captive portal on startup...");
    if let Ok(true) = check_and_login(&username, &password).await {
        sleep_duration = tokio::time::Duration::from_secs(MAX_DELAY_SECS);
    }

    info!("Starting hybrid network watcher and polling loop...");

    loop {
        info!("Next poll in {:.0?} seconds.", sleep_duration.as_secs_f32());

        tokio::select! {
            biased;

            Some(_) = rx.recv() => {
                info!("Received signal from network watcher. Triggering immediate check.");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                if let Ok(true) = check_and_login(&username, &password).await {
                    sleep_duration = tokio::time::Duration::from_secs(MAX_DELAY_SECS);
                } else {
                    sleep_duration = tokio::time::Duration::from_secs(MIN_DELAY_SECS);
                }
            },

            _ = tokio::time::sleep(sleep_duration) => {
                info!("Polling interval elapsed. Checking for captive portal...");
                match check_and_login(&username, &password).await {
                    Ok(true) => {
                        sleep_duration = tokio::time::Duration::from_secs(MAX_DELAY_SECS);
                    },
                    Ok(false) => {
                        let current_secs = sleep_duration.as_secs();
                        let next_secs = (current_secs / 2).max(MIN_DELAY_SECS);
                        sleep_duration = tokio::time::Duration::from_secs(next_secs);
                    },
                    Err(_) => {
                        sleep_duration = tokio::time::Duration::from_secs(MIN_DELAY_SECS);
                    }
                }
            },
        }
    }
}

async fn health_check() -> Result<()> {
    info!("Performing health check...");

    match get_credentials() {
        Ok((username, _)) => {
            info!("✓ Credentials found for user: {username}");
        }
        Err(e) => {
            error!("✗ Failed to retrieve credentials: {e}");
            return Err(e);
        }
    }

    match captive_portal::check_captive_portal().await {
        Ok(Some((url, magic))) => {
            info!("✓ Captive portal detected at: {url}");
            info!("✓ Magic value extracted: {magic}");
        }
        Ok(None) => {
            info!("✓ No captive portal detected (internet is accessible)");
        }
        Err(e) => {
            error!("✗ Network check failed: {e}");
            return Err(e);
        }
    }

    info!("Health check completed successfully");
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("setup") => {
            if let Err(e) = setup().await {
                error!("Setup failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        Some("health") | Some("check") => {
            if let Err(e) = health_check().await {
                error!("Health check failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        Some("--help") | Some("-h") => {
            println!("Auto Captive Portal Login Service");
            println!();
            println!("USAGE:");
            println!("    acp [SUBCOMMAND]");
            println!();
            println!("SUBCOMMANDS:");
            println!("    setup    Configure credentials and install service");
            println!("    health   Perform health check");
            println!("    help     Print this help message");
            println!();
            println!("Running without arguments starts the service.");
            return;
        }
        Some(_) => {
            error!("Unknown command. Use 'acp --help' for usage information.");
            std::process::exit(1);
        }
        None => {
            // Default: run the service
        }
    }

    if let Err(e) = run().await {
        error!("Application error: {e}");
        std::process::exit(1);
    }
}
