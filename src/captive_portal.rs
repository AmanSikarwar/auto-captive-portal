use crate::error::{AppError, Result};
use log::{error, info, warn};
use regex::Regex;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

const MAX_LOGIN_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_SECS: u64 = 2;
const LOGOUT_URL: &str = "https://login.iitmandi.ac.in:1003/logout?";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_CONNECTIVITY_CHECK_URL: &str = "http://clients3.google.com/generate_204";

fn get_connectivity_check_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        std::env::var("ACP_CONNECTIVITY_URL")
            .unwrap_or_else(|_| DEFAULT_CONNECTIVITY_CHECK_URL.to_string())
    })
}

fn portal_url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"window\.location="([^"]*)""#).unwrap())
}

fn magic_value_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"<input type="hidden" name="magic" value="([^"]*)">"#).unwrap())
}

fn get_client() -> Result<reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(2)
            .build()
            .expect("Failed to create HTTP client")
    });
    Ok(CLIENT.get().unwrap().clone())
}

pub async fn logout() -> Result<()> {
    info!(
        "Attempting to logout from captive portal at: {}",
        LOGOUT_URL
    );

    let client = get_client()?;

    match client.get(LOGOUT_URL).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() || status.is_redirection() {
                info!("Logout request completed. Status: {}", status);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                Ok(())
            } else {
                warn!("Logout request returned non-success status: {}", status);
                Ok(())
            }
        }
        Err(e) => {
            warn!("Logout request failed (this is often expected): {}", e);
            Ok(())
        }
    }
}

pub async fn verify_internet_connectivity() -> Result<bool> {
    let check_url = get_connectivity_check_url();
    let client = get_client()?;

    match client.get(check_url).send().await {
        Ok(resp) if resp.status() == StatusCode::NO_CONTENT => {
            info!("Internet connectivity verified: received expected 204 response");
            Ok(true)
        }
        Ok(resp) => {
            warn!(
                "Unexpected response from connectivity check: {}",
                resp.status()
            );
            Ok(false)
        }
        Err(e) => {
            warn!("Failed to verify connectivity: {e}");
            Err(AppError::Network(e))
        }
    }
}

pub async fn login(portal_url: &str, username: &str, password: &str, magic: &str) -> Result<()> {
    let login_url = if portal_url.contains("login.iitmandi.ac.in") {
        "https://login.iitmandi.ac.in:1003/portal?"
    } else {
        portal_url
    };

    info!("Attempting to login to captive portal via POST request at: {login_url}");

    let client = get_client()?;

    let mut form_data = HashMap::new();
    form_data.insert("username", username);
    form_data.insert("password", password);
    form_data.insert("4Tredir", login_url);
    form_data.insert("magic", magic);

    let resp = client.post(login_url).form(&form_data).send().await?;
    let status = resp.status();

    if !status.is_success() && !status.is_redirection() {
        error!("Login request failed. Status: {status}");
        let error_body = resp.text().await?;
        error!("Response Body: {error_body:?}");
        return Err(AppError::LoginFailed(format!(
            "Login failed with status code: {status}"
        )));
    }

    info!("Login request sent successfully. Status: {status}");
    info!("Verifying actual internet connectivity...");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    match verify_internet_connectivity().await {
        Ok(true) => {
            info!("Login successful: internet connectivity confirmed");
            Ok(())
        }
        Ok(false) => {
            error!("Login verification failed: portal returned success but no internet access");
            Err(AppError::LoginFailed(
                "Portal accepted credentials but internet is not accessible. \
                This may indicate incorrect credentials or portal issues."
                    .to_string(),
            ))
        }
        Err(e) => {
            warn!("Could not verify connectivity after login: {e}");
            info!("Assuming login succeeded despite verification failure");
            Ok(())
        }
    }
}

pub async fn login_with_retry(
    portal_url: &str,
    username: &str,
    password: &str,
    magic: &str,
) -> Result<()> {
    let mut last_error: Option<AppError> = None;

    for attempt in 1..=MAX_LOGIN_RETRIES {
        info!("Login attempt {}/{}", attempt, MAX_LOGIN_RETRIES);

        match login(portal_url, username, password, magic).await {
            Ok(()) => {
                if attempt > 1 {
                    info!("Login succeeded after {} attempt(s)", attempt);
                }
                return Ok(());
            }
            Err(e) => {
                error!("Login attempt {} failed: {}", attempt, e);
                last_error = Some(e);

                if attempt < MAX_LOGIN_RETRIES {
                    info!("Logging out before retry to ensure clean state...");
                    let _ = logout().await;

                    let delay_secs = INITIAL_RETRY_DELAY_SECS * 2u64.pow(attempt - 1);
                    warn!("Retrying login in {} seconds...", delay_secs);
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
                } else {
                    error!(
                        "All {} login attempts failed. Giving up.",
                        MAX_LOGIN_RETRIES
                    );
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::LoginFailed("Login failed after all retry attempts".to_string())
    }))
}

pub fn extract_captive_portal_url(html: &str) -> Option<String> {
    portal_url_regex()
        .captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_magic_value(html: &str) -> Option<String> {
    magic_value_regex()
        .captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .filter(|s| !s.is_empty()) // Filter out empty magic values
}

pub async fn check_captive_portal() -> Result<Option<(String, String)>> {
    let check_url = get_connectivity_check_url();
    let max_check_retries = 2;
    let mut last_error: Option<reqwest::Error> = None;
    let client = get_client()?;

    for attempt in 1..=max_check_retries {
        match client.get(check_url).send().await {
            Ok(connectivity_check_resp) => {
                return check_portal_response(connectivity_check_resp, &client).await;
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < max_check_retries {
                    warn!(
                        "Portal check attempt {} failed. Retrying in 1 second...",
                        attempt
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                } else {
                    error!("All {} portal check attempts failed", max_check_retries);
                }
            }
        }
    }

    Err(AppError::Network(last_error.unwrap()))
}

async fn check_portal_response(
    google_check_resp: reqwest::Response,
    client: &reqwest::Client,
) -> Result<Option<(String, String)>> {
    let status = google_check_resp.status();
    match status {
        StatusCode::NO_CONTENT => {
            info!("No captive portal detected: received expected 204 response");
            Ok(None)
        }
        StatusCode::OK => {
            let html: String = google_check_resp.text().await?;
            let captive_portal_url_option: Option<String> = extract_captive_portal_url(&html);

            if let Some(captive_portal_url) = captive_portal_url_option {
                info!("Captive portal URL detected: {captive_portal_url}");
                let portal_page_resp = client.get(&captive_portal_url).send().await?;
                if portal_page_resp.status().is_success() {
                    let portal_html = portal_page_resp.text().await?;
                    let magic_value_option = extract_magic_value(&portal_html);
                    if let Some(magic_value) = magic_value_option {
                        info!("Extracted magic value: {magic_value}");
                        Ok(Some((captive_portal_url, magic_value)))
                    } else {
                        error!(
                            "Could not extract magic value from captive portal page (empty or missing)."
                        );
                        Err(AppError::LoginFailed(
                            "Failed to extract magic value from portal page".to_string(),
                        ))
                    }
                } else {
                    let status = portal_page_resp.status();
                    error!(
                        "Failed to fetch captive portal page to extract magic value. Status: {}",
                        status
                    );
                    Err(AppError::LoginFailed(format!(
                        "Portal page returned error status: {}",
                        status
                    )))
                }
            } else {
                warn!("Received 200 response but could not extract portal URL from response");
                Ok(None)
            }
        }
        status if status.is_client_error() || status.is_server_error() => Err(AppError::Network(
            google_check_resp.error_for_status().unwrap_err(),
        )),
        _ => {
            warn!("Unexpected status code from connectivity check: {}", status);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_captive_portal_url_valid() {
        let html = r#"<script>window.location="https://login.iitmandi.ac.in:1003/portal"</script>"#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("https://login.iitmandi.ac.in:1003/portal".to_string())
        );
    }

    #[test]
    fn test_extract_captive_portal_url_missing() {
        let html = r#"<html><body>No redirect here</body></html>"#;
        assert_eq!(extract_captive_portal_url(html), None);
    }

    #[test]
    fn test_extract_magic_value_valid() {
        let html = r#"<form><input type="hidden" name="magic" value="abc123def456"></form>"#;
        assert_eq!(extract_magic_value(html), Some("abc123def456".to_string()));
    }

    #[test]
    fn test_extract_magic_value_missing() {
        let html = r#"<form><input type="text" name="username"></form>"#;
        assert_eq!(extract_magic_value(html), None);
    }

    #[test]
    fn test_extract_magic_value_empty() {
        let html = r#"<input type="hidden" name="magic" value="">"#;
        assert_eq!(extract_magic_value(html), None);
    }

    #[test]
    fn test_extract_portal_url_with_special_chars() {
        let html = r#"window.location="https://portal.example.com/login?redirect=http://example.com&token=xyz""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some(
                "https://portal.example.com/login?redirect=http://example.com&token=xyz"
                    .to_string()
            )
        );
    }
}
