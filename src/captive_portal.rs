use crate::error::{AppError, Result};
use log::{error, info};
use regex::Regex;
use reqwest::StatusCode;
use std::collections::HashMap;

pub async fn login(portal_url: &str, username: &str, password: &str, magic: &str) -> Result<()> {
    let login_url = if portal_url.contains("login.iitmandi.ac.in") {
        "https://login.iitmandi.ac.in:1003/portal?"
    } else {
        portal_url
    };

    info!("Attempting to login to captive portal via POST request at: {login_url}");

    let client = reqwest::Client::new();

    let mut form_data = HashMap::new();
    form_data.insert("username", username);
    form_data.insert("password", password);
    form_data.insert("4Tredir", login_url);
    form_data.insert("magic", magic);

    let resp = client.post(login_url).form(&form_data).send().await?;

    if resp.status().is_success() || resp.status().is_redirection() {
        info!("Successfully sent login request. Status: {}", resp.status());
        // For POST-based login, successful submission is often enough.
        // We might need to refine success check based on actual portal behavior
        Ok(())
    } else {
        let status = resp.status();
        error!("Login request failed. Status: {status}");
        let error_body = resp.text().await?;
        error!("Response Body: {error_body:?}");
        Err(AppError::LoginFailed(format!(
            "Login failed with status code: {status}"
        )))
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
    let client: reqwest::Client = reqwest::Client::new();
    let google_check_resp: reqwest::Response = client.get(google_check_url).send().await?;

    match google_check_resp.status() {
        StatusCode::NO_CONTENT => Ok(None),
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
