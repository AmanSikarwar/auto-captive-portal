use crate::error::{AppError, Result};
use log::{error, info, warn};
use regex::Regex;
use reqwest::StatusCode;
use std::collections::HashMap;

pub async fn verify_internet_connectivity() -> Result<bool> {
    let google_check_url: &str = "http://clients3.google.com/generate_204";
    let client: reqwest::Client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(google_check_url).send().await {
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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

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

pub fn extract_captive_portal_url(html: &str) -> Option<String> {
    let re: Regex = Regex::new(r#"window\.location="([^"]*)""#).unwrap();
    re.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_magic_value(html: &str) -> Option<String> {
    let re: Regex = Regex::new(r#"<input type="hidden" name="magic" value="([^"]*)">"#).unwrap();
    re.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

pub async fn check_captive_portal() -> Result<Option<(String, String)>> {
    let google_check_url: &str = "http://clients3.google.com/generate_204";
    let client: reqwest::Client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let google_check_resp: reqwest::Response = client.get(google_check_url).send().await?;

    match google_check_resp.status() {
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
                        error!("Could not extract magic value from captive portal page.");
                        Ok(None)
                    }
                } else {
                    error!(
                        "Failed to fetch captive portal page to extract magic value. Status: {}",
                        portal_page_resp.status()
                    );
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        _ => Err(AppError::Network(
            google_check_resp.error_for_status().unwrap_err(),
        )),
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
        assert_eq!(extract_magic_value(html), Some("".to_string()));
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
