mod captive_portal;
mod credentials;
mod daemon;
mod error;
mod logging;
mod notifications;
mod service;
mod state;

use crate::credentials::SERVICE_NAME;
use clap::{Parser, Subcommand};
use console::Term;
use error::{AppError, Result};
use log::{error, info};
use service::ServiceManager;
use std::env;
use std::fs;

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

    let executable_path = env::current_exe()?;
    let service_manager = ServiceManager::new(executable_path);

    credentials::store_credentials(&username, &password)?;
    service_manager.create_service()?;

    info!("Setup completed successfully.");
    Ok(())
}

async fn update_credentials(username: Option<String>) -> Result<()> {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║     Update Credentials - Auto Captive Portal         ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    match credentials::get_credentials() {
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

    credentials::store_credentials(&username, &password)?;

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
        match credentials::clear_credentials() {
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

async fn health_check() -> Result<()> {
    info!("Performing health check...");

    match credentials::get_credentials() {
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

    let creds_status = match credentials::get_credentials() {
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

    if let Ok(state_path) = state::get_state_file_path()
        && let Ok(contents) = fs::read_to_string(&state_path)
        && let Ok(service_state) = serde_json::from_str::<state::ServiceState>(&contents)
    {
        println!("\n─────────────────────────────────────────────────────");

        if let Some(ts) = service_state.last_check_timestamp {
            println!("Last Check:         {}", state::format_duration_ago(ts));
        }

        if let Some(ts) = service_state.last_successful_login_timestamp {
            println!("Last Login:         {}", state::format_duration_ago(ts));
        }

        if let Some(portal) = service_state.last_portal_detected {
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

            let username = prompt_input("Enter LDAP Username: ", false).map_err(AppError::from)?;
            let password = prompt_input("Enter LDAP Password: ", true).map_err(AppError::from)?;
            credentials::store_credentials(&username, &password)?;

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
        Some(Commands::Run) => daemon::run().await,
        #[cfg(target_os = "windows")]
        Some(Commands::Service(cmd)) => handle_windows_service_command(cmd).await,
        None => daemon::run().await,
    };

    if let Err(e) = result {
        error!("Error: {e}");
        std::process::exit(1);
    }
}
