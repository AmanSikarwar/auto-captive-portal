mod captive_portal;
mod error;
mod logging;
mod notifications;
mod service;

use clap::{Parser, Subcommand};
use console::Term;
use error::{AppError, Result};
use keyring::Entry;
use log::{error, info};
use service::{SERVICE_NAME, ServiceManager};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "acp")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Setup {
        #[arg(short, long)]
        username: Option<String>,
    },

    UpdateCredentials {
        #[arg(short, long)]
        username: Option<String>,
    },

    Status,

    Health,

    Logout {
        #[arg(short, long)]
        clear_credentials: bool,
    },

    #[command(hide = true)]
    Run,

    #[cfg(target_os = "windows")]
    #[command(subcommand)]
    Service(ServiceCommands),
}

#[cfg(target_os = "windows")]
#[derive(Subcommand)]
enum ServiceCommands {
    Install,
    Uninstall,
    Start,
    Stop,
}

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

fn clear_credentials() -> Result<()> {
    let username_entry = Entry::new(SERVICE_NAME, "ldap_username").map_err(AppError::from)?;
    let password_entry = Entry::new(SERVICE_NAME, "ldap_password").map_err(AppError::from)?;

    let _ = username_entry.delete_credential();
    let _ = password_entry.delete_credential();

    Ok(())
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
            update_state_file(Some(&url), false).ok();
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

async fn setup(username: Option<String>) -> Result<()> {
    info!("Starting setup for Auto Captive Portal...");

    let username = match username {
        Some(u) => u,
        None => prompt_input("Enter LDAP Username: ", false).map_err(AppError::from)?,
    };
    let password: String = prompt_input("Enter LDAP Password: ", true).map_err(AppError::from)?;

    if username.trim().is_empty() {
        return Err(AppError::Service("Username cannot be empty".to_string()));
    }
    if password.trim().is_empty() {
        return Err(AppError::Service("Password cannot be empty".to_string()));
    }

    let executable_path: std::path::PathBuf = env::current_exe()?;
    let service_manager: ServiceManager = ServiceManager::new(executable_path);

    service_manager.store_credentials(&username, &password)?;
    service_manager.create_service()?;

    info!("Setup completed successfully.");
    Ok(())
}

async fn update_credentials(username: Option<String>) -> Result<()> {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║     Update Credentials - Auto Captive Portal         ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    match get_credentials() {
        Ok((current_username, _)) => {
            println!("Current username: {}\n", current_username);
        }
        Err(_) => {
            println!("⚠  No credentials found. Use 'acp setup' instead.\n");
            return Err(AppError::Service(
                "No existing credentials found".to_string(),
            ));
        }
    }

    let username = match username {
        Some(u) => u,
        None => prompt_input("Enter new LDAP Username: ", false).map_err(AppError::from)?,
    };
    let password: String =
        prompt_input("Enter new LDAP Password: ", true).map_err(AppError::from)?;

    if username.trim().is_empty() {
        return Err(AppError::Service("Username cannot be empty".to_string()));
    }

    if password.trim().is_empty() {
        return Err(AppError::Service("Password cannot be empty".to_string()));
    }

    println!("\nValidating credentials...");
    match captive_portal::check_captive_portal().await {
        Ok(Some((url, magic))) => {
            println!("Captive portal detected. Testing credentials...");

            println!("Logging out from current session...");
            let _ = captive_portal::logout().await;

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            match captive_portal::login(&url, &username, &password, &magic).await {
                Ok(_) => {
                    println!("✓ Credentials validated successfully!");
                }
                Err(e) => {
                    println!("✗ Credential validation failed: {}", e);
                    println!("\nCredentials may be incorrect. Store anyway? (y/N): ");
                    let response = prompt_input("", false).map_err(AppError::from)?;
                    if response.to_lowercase() != "y" {
                        println!("Credential update cancelled.");
                        return Ok(());
                    }
                }
            }
        }
        Ok(None) => {
            println!("⚠  No captive portal detected. Credentials cannot be validated.");
            println!("Storing credentials anyway...");
        }
        Err(e) => {
            println!(
                "⚠  Portal check failed: {}. Storing credentials anyway...",
                e
            );
        }
    }

    let executable_path: std::path::PathBuf = env::current_exe()?;
    let service_manager: ServiceManager = ServiceManager::new(executable_path);
    service_manager.store_credentials(&username, &password)?;

    println!("\n✓ Credentials updated successfully!");
    println!("\nℹ  The service will use the new credentials on the next login attempt.");
    println!();

    Ok(())
}

async fn logout_command(clear_creds: bool) -> Result<()> {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║     Logout - Auto Captive Portal                     ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    println!("Logging out from captive portal...");

    match captive_portal::logout().await {
        Ok(_) => {
            println!("✓ Logout request sent successfully.");
            notifications::send_notification("Logged out from captive portal.").await;
        }
        Err(e) => {
            println!("⚠  Logout request failed: {}", e);
            println!("   (This may be expected if you're not currently logged in)");
        }
    }

    if clear_creds {
        println!("\nClearing stored credentials...");
        match clear_credentials() {
            Ok(_) => {
                println!("✓ Credentials cleared successfully.");
            }
            Err(e) => {
                println!("✗ Failed to clear credentials: {}", e);
                return Err(e);
            }
        }
    }

    println!();
    Ok(())
}

async fn run() -> Result<()> {
    let (username, password) = get_credentials()?;

    const MAX_DELAY_SECS: u64 = 1800;
    const MIN_DELAY_SECS: u64 = 10;
    const CHANNEL_CAPACITY: usize = 10; // Bounded channel to prevent memory issues
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
            if tx.try_send(()).is_err() {}
        } else {
            info!("Ignoring irrelevant network change (e.g., interface or IP removed).");
        }
    })
    .map_err(|e| AppError::Service(e.to_string()))?;

    info!("Performing initial check for captive portal on startup...");
    if let Ok(true) = check_and_login(&username, &password).await {
        sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
        update_state_file(None, true).ok();
    }

    info!("Starting hybrid network watcher and polling loop...");

    loop {
        info!("Next poll in {:.0?} seconds.", sleep_duration.as_secs_f32());

        tokio::select! {
            biased;

            Some(_) = rx.recv() => {
                info!("Received signal from network watcher. Triggering immediate check.");
                tokio::time::sleep(Duration::from_secs(3)).await;

                if let Ok(true) = check_and_login(&username, &password).await {
                    sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                    update_state_file(None, true).ok();
                } else {
                    sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                    update_state_file(None, false).ok();
                }
            },

            _ = tokio::time::sleep(sleep_duration) => {
                info!("Polling interval elapsed. Checking for captive portal...");
                match check_and_login(&username, &password).await {
                    Ok(true) => {
                        sleep_duration = Duration::from_secs(MAX_DELAY_SECS);
                        update_state_file(None, true).ok();
                    },
                    Ok(false) => {
                        let current_secs = sleep_duration.as_secs();
                        let next_secs = (current_secs / 2).max(MIN_DELAY_SECS);
                        sleep_duration = Duration::from_secs(next_secs);
                        update_state_file(None, false).ok();
                    },
                    Err(_) => {
                        sleep_duration = Duration::from_secs(MIN_DELAY_SECS);
                        update_state_file(None, false).ok();
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

fn get_state_file_path() -> Result<PathBuf> {
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

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ServiceState {
    last_check_timestamp: Option<u64>,
    last_successful_login_timestamp: Option<u64>,
    last_portal_detected: Option<String>,
}

fn update_state_file(portal_url: Option<&str>, login_success: bool) -> Result<()> {
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

fn format_duration_ago(timestamp: u64) -> String {
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

fn check_service_running() -> (bool, String) {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("launchctl")
            .args(["list", SERVICE_NAME])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("PID") && !stdout.contains("PID\" = 0") {
                    (true, "Running".to_string())
                } else {
                    (false, "Not Running".to_string())
                }
            }
            _ => (false, "Not Running".to_string()),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("systemctl")
            .args(["--user", "is-active", SERVICE_NAME])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if stdout == "active" {
                    (true, "Running".to_string())
                } else {
                    (false, stdout)
                }
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (
                    false,
                    if stdout.is_empty() {
                        "Not Running".to_string()
                    } else {
                        stdout
                    },
                )
            }
            Err(_) => (false, "Unknown".to_string()),
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let output = Command::new("sc").args(["query", SERVICE_NAME]).output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("RUNNING") {
                    (true, "Running".to_string())
                } else if stdout.contains("STOPPED") {
                    (false, "Stopped".to_string())
                } else {
                    (false, "Unknown".to_string())
                }
            }
            _ => (false, "Not Installed".to_string()),
        }
    }
}

async fn show_status() -> Result<()> {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║     Auto Captive Portal - Service Status             ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let creds_status = match get_credentials() {
        Ok((username, _)) => {
            println!("Credentials:        ✓ Configured (user: {})", username);
            true
        }
        Err(_) => {
            println!("Credentials:        ✗ Not configured (run 'acp setup')");
            false
        }
    };

    let (is_running, service_state) = check_service_running();
    if is_running {
        println!("Service:            ✓ {}", service_state);
    } else {
        println!("Service:            ✗ {}", service_state);
    }

    print!("Internet:           ");
    match captive_portal::verify_internet_connectivity().await {
        Ok(true) => println!("✓ Connected"),
        Ok(false) | Err(_) => println!("✗ Not connected"),
    }

    print!("Portal Status:      ");
    match captive_portal::check_captive_portal().await {
        Ok(Some((url, _))) => {
            println!("⚠ Detected");
            println!("Portal URL:         {}", url);
        }
        Ok(None) => println!("✓ Not detected"),
        Err(_) => println!("✗ Check failed"),
    }

    if let Ok(state_path) = get_state_file_path()
        && let Ok(contents) = fs::read_to_string(&state_path)
        && let Ok(state) = serde_json::from_str::<ServiceState>(&contents)
    {
        println!("\n─────────────────────────────────────────────────────");

        if let Some(ts) = state.last_check_timestamp {
            println!("Last Check:         {}", format_duration_ago(ts));
        }

        if let Some(ts) = state.last_successful_login_timestamp {
            println!("Last Login:         {}", format_duration_ago(ts));
        }

        if let Some(portal) = state.last_portal_detected {
            println!("Last Portal:        {}", portal);
        }
    }

    println!("\n─────────────────────────────────────────────────────");

    if !creds_status {
        println!("\n⚠  Run 'acp setup' to configure credentials");
    } else if !is_running {
        println!("\n⚠  Service is not running. Check system logs:");
        #[cfg(target_os = "macos")]
        println!("   launchctl list | grep {}", SERVICE_NAME);
        #[cfg(target_os = "linux")]
        println!("   systemctl --user status {}", SERVICE_NAME);
        #[cfg(target_os = "windows")]
        println!("   sc query {}", SERVICE_NAME);
    }

    println!();
    Ok(())
}

#[cfg(target_os = "windows")]
async fn handle_windows_service_command(cmd: ServiceCommands) -> Result<()> {
    use service::windows_service_control;

    match cmd {
        ServiceCommands::Install => {
            println!("Installing Windows service...");
            let executable_path = env::current_exe()?;
            let service_manager = ServiceManager::new(executable_path);

            // Prompt for credentials first
            let username = prompt_input("Enter LDAP Username: ", false).map_err(AppError::from)?;
            let password = prompt_input("Enter LDAP Password: ", true).map_err(AppError::from)?;
            service_manager.store_credentials(&username, &password)?;

            // Prompt for Windows service account
            println!("\nWindows Service Account Configuration");
            println!("Leave blank to run as the current user.");
            let account = prompt_input("Service account (e.g., .\\username): ", false)
                .map_err(AppError::from)?;
            let account_password = if account.is_empty() {
                None
            } else {
                Some(prompt_input("Service account password: ", true).map_err(AppError::from)?)
            };

            service_manager.create_service_with_account(
                if account.is_empty() {
                    None
                } else {
                    Some(&account)
                },
                account_password.as_deref(),
            )?;

            println!("✓ Windows service installed successfully.");
            Ok(())
        }
        ServiceCommands::Uninstall => {
            println!("Uninstalling Windows service...");
            windows_service_control::uninstall_service()?;
            println!("✓ Windows service uninstalled successfully.");
            Ok(())
        }
        ServiceCommands::Start => {
            println!("Starting Windows service...");
            windows_service_control::start_service()?;
            println!("✓ Windows service started.");
            Ok(())
        }
        ServiceCommands::Stop => {
            println!("Stopping Windows service...");
            windows_service_control::stop_service()?;
            println!("✓ Windows service stopped.");
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let is_service = matches!(cli.command, Some(Commands::Run));

    if let Err(e) = logging::init_logging(is_service) {
        eprintln!("Warning: Failed to initialize logging: {}", e);
    }

    let result = match cli.command {
        Some(Commands::Setup { username }) => setup(username).await,
        Some(Commands::UpdateCredentials { username }) => update_credentials(username).await,
        Some(Commands::Status) => show_status().await,
        Some(Commands::Health) => health_check().await,
        Some(Commands::Logout { clear_credentials }) => logout_command(clear_credentials).await,
        Some(Commands::Run) => run().await,
        #[cfg(target_os = "windows")]
        Some(Commands::Service(cmd)) => handle_windows_service_command(cmd).await,
        None => {
            // Default: run the service
            run().await
        }
    };

    if let Err(e) = result {
        error!("Error: {e}");
        std::process::exit(1);
    }
}
