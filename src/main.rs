mod captive_portal;
mod error;
mod notifications;
mod service;

use error::{AppError, Result};
use keyring::Entry;
use service::{ServiceManager, SERVICE_NAME};
use std::{
    env,
    io::{self, Write},
};

fn get_credentials() -> Result<(String, String)> {
    let username_entry = Entry::new(SERVICE_NAME, "ldap_username").map_err(AppError::from)?;
    let password_entry = Entry::new(SERVICE_NAME, "ldap_password").map_err(AppError::from)?;
    Ok((
        username_entry.get_password().map_err(AppError::from)?,
        password_entry.get_password().map_err(AppError::from)?,
    ))
}

fn prompt_input(prompt: &str) -> std::result::Result<String, std::io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

async fn setup() -> Result<()> {
    println!("Setting up Auto Captive Portal...");

    let username = prompt_input("Enter LDAP Username: ").map_err(AppError::from)?;
    let password = prompt_input("Enter LDAP Password: ").map_err(AppError::from)?;

    let executable_path = env::current_exe()?;
    let service_manager = ServiceManager::new(executable_path);

    service_manager.store_credentials(&username, &password)?;
    service_manager.create_service()?;

    println!("Setup completed successfully!");
    Ok(())
}

async fn run() -> Result<()> {
    let (username, password) = get_credentials()?;

    loop {
        match captive_portal::check_captive_portal().await {
            Ok(Some(url)) => {
                println!("Captive portal detected at {}", url);
                if let Err(e) = captive_portal::login(&url, &username, &password).await {
                    eprintln!("Login failed: {}", e);
                    service::restart_service().await?;
                } else {
                    notifications::send_notification(
                        "Captive portal detected and logged in successfully",
                    )
                    .await;
                }
            }
            Ok(None) => println!("No captive portal detected"),
            Err(e) => {
                eprintln!("Portal check failed: {}", e);
                service::restart_service().await?;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

#[tokio::main]
async fn main() {
    if env::args().nth(1).as_deref() == Some("setup") {
        if let Err(e) = setup().await {
            eprintln!("Setup failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = run().await {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
