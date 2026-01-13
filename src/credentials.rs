use crate::error::{AppError, Result};
use keyring::Entry;
use log::info;

pub const SERVICE_NAME: &str = if cfg!(target_os = "macos") {
    "com.user.acp"
} else {
    "acp"
};

pub fn store_credentials(username: &str, password: &str) -> Result<()> {
    let username_entry = Entry::new(SERVICE_NAME, "ldap_username")?;
    username_entry.set_password(username)?;

    let password_entry = Entry::new(SERVICE_NAME, "ldap_password")?;
    password_entry.set_password(password)?;

    info!("Credentials stored successfully");
    Ok(())
}

pub fn get_credentials() -> Result<(String, String)> {
    let username_entry = Entry::new(SERVICE_NAME, "ldap_username").map_err(AppError::from)?;
    let password_entry = Entry::new(SERVICE_NAME, "ldap_password").map_err(AppError::from)?;
    Ok((
        username_entry.get_password().map_err(AppError::from)?,
        password_entry.get_password().map_err(AppError::from)?,
    ))
}

pub fn clear_credentials() -> Result<()> {
    let username_entry = Entry::new(SERVICE_NAME, "ldap_username").map_err(AppError::from)?;
    let password_entry = Entry::new(SERVICE_NAME, "ldap_password").map_err(AppError::from)?;

    let _ = username_entry.delete_credential();
    let _ = password_entry.delete_credential();

    info!("Credentials cleared");
    Ok(())
}
