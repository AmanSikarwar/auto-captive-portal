use crate::captive_portal;
use crate::credentials;
use crate::error::{AppError, Result};
use crate::notifications;
use crate::state;
use log::{error, info, warn};
use std::time::Duration;
use tokio::sync::mpsc;

const MAX_DELAY_SECS: u64 = 1800;
const MIN_DELAY_SECS: u64 = 10;
const CHANNEL_CAPACITY: usize = 10;

#[cfg(unix)]
async fn shutdown_signal() -> Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| {
            error!("Failed to create SIGTERM handler: {}", e);
            e
        })?;
    let mut sigint = signal(SignalKind::interrupt())
        .map_err(|e| {
            error!("Failed to create SIGINT handler: {}", e);
            e
        })?;

    tokio::select! {
        _ = sigterm.recv() => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT, initiating graceful shutdown...");
        }
    }

    Ok(())
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<()> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| {
            error!("Failed to listen for Ctrl+C: {}", e);
            e
        })?;
    info!("Received Ctrl+C, initiating graceful shutdown...");
    Ok(())
}

pub async fn check_and_login(username: &str, password: &str) -> Result<bool> {
    match captive_portal::check_captive_portal().await {
        Ok(Some((url, magic))) => {
            info!("Captive portal detected at {url}");
            state::update_state_file(Some(&url), false).ok();
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

pub async fn run() -> Result<()> {
    let (username, password) = credentials::get_credentials()?;
    run_with_credentials(&username, &password).await
}

pub async fn run_with_credentials(username: &str, password: &str) -> Result<()> {
    let mut sleep_duration = Duration::from_secs(MIN_DELAY_SECS);

    let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);

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
            if tx.try_send(()).is_err() {
                warn!("Failed to send network change signal - channel full or closed");
            }
        } else {
            info!("Ignoring irrelevant network change (e.g., interface or IP removed).");
        }
    })
    .map_err(|e| AppError::Service(e.to_string()))?;

    info!("Performing initial check for captive portal on startup...");
    if let Ok(true) = check_and_login(username, password).await {
        sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
        state::update_state_file(None, true).ok();
    }

    info!("Starting hybrid network watcher and polling loop...");

    loop {
        info!("Next poll in {:.0?} seconds.", sleep_duration.as_secs_f32());

        tokio::select! {
            biased;

            result = shutdown_signal() => {
                match result {
                    Ok(_) => {
                        info!("Shutdown signal received, updating state and exiting...");
                        state::update_state_file(None, false).ok();
                    }
                    Err(e) => {
                        error!("Error setting up shutdown signal handler: {}", e);
                        state::update_state_file(None, false).ok();
                    }
                }
                break;
            },

            Some(_) = rx.recv() => {
                info!("Received signal from network watcher. Triggering immediate check.");
                tokio::time::sleep(Duration::from_secs(3)).await;

                if let Ok(true) = check_and_login(username, password).await {
                    sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                    state::update_state_file(None, true).ok();
                } else {
                    sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                    state::update_state_file(None, false).ok();
                }
            },

            _ = tokio::time::sleep(sleep_duration) => {
                info!("Polling interval elapsed. Checking for captive portal...");
                match check_and_login(username, password).await {
                    Ok(true) => {
                        sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                        state::update_state_file(None, true).ok();
                    },
                    Ok(false) => {
                        let current_secs = sleep_duration.as_secs();
                        let next_secs = (current_secs / 2).max(MIN_DELAY_SECS);
                        sleep_duration = Duration::from_secs(next_secs);
                        state::update_state_file(None, false).ok();
                    },
                    Err(_) => {
                        sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                        state::update_state_file(None, false).ok();
                    }
                }
            },
        }
    }

    info!("Daemon shutdown complete.");
    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn run_with_shutdown(
    username: &str,
    password: &str,
    shutdown_rx: std::sync::mpsc::Receiver<()>,
) {
    let mut sleep_duration = Duration::from_secs(MIN_DELAY_SECS);

    let (tx, mut rx) = mpsc::channel::<()>(CHANNEL_CAPACITY);

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
            if tx.try_send(()).is_err() {
                warn!("Failed to send network change signal - channel full or closed");
            }
        }
    }) {
        Ok(handle) => handle,
        Err(e) => {
            error!("Failed to start network watcher: {}", e);
            return;
        }
    };

    info!("Performing initial captive portal check...");
    if let Ok(true) = check_and_login(username, password).await {
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

                if let Ok(true) = check_and_login(username, password).await {
                    sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                } else {
                    sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                }
            },

            _ = tokio::time::sleep(sleep_duration) => {
                info!("Polling interval elapsed");
                match check_and_login(username, password).await {
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
