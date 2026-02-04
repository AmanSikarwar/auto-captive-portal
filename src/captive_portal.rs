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
            Ok(google_check_resp) => {
                return check_portal_response(google_check_resp, &client).await;
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

    #[test]
    fn test_extract_portal_url_multiple_matches() {
        let html = r#"
            <script>window.location="https://first.com"</script>
            <script>window.location="https://second.com"</script>
        "#;
        // Should extract the first match
        let result = extract_captive_portal_url(html);
        assert!(result.is_some());
        assert_eq!(result, Some("https://first.com".to_string()));
    }

    #[test]
    fn test_extract_portal_url_empty_string() {
        let html = "";
        assert_eq!(extract_captive_portal_url(html), None);
    }

    #[test]
    fn test_extract_portal_url_with_whitespace() {
        let html = r#"  window.location="https://portal.test.com"  "#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("https://portal.test.com".to_string())
        );
    }

    #[test]
    fn test_extract_magic_value_multiple_inputs() {
        let html = r#"
            <input type="hidden" name="other" value="other123">
            <input type="hidden" name="magic" value="correct_magic">
            <input type="hidden" name="another" value="another456">
        "#;
        assert_eq!(
            extract_magic_value(html),
            Some("correct_magic".to_string())
        );
    }

    #[test]
    fn test_extract_magic_value_with_special_characters() {
        let html =
            r#"<input type="hidden" name="magic" value="abc-123_DEF.456+789=xyz/test@domain">"#;
        assert_eq!(
            extract_magic_value(html),
            Some("abc-123_DEF.456+789=xyz/test@domain".to_string())
        );
    }

    #[test]
    fn test_extract_magic_value_empty_html() {
        let html = "";
        assert_eq!(extract_magic_value(html), None);
    }

    #[test]
    fn test_extract_magic_value_whitespace_value() {
        let html = r#"<input type="hidden" name="magic" value="   ">"#;
        // Should still extract non-empty whitespace
        assert_eq!(extract_magic_value(html), Some("   ".to_string()));
    }

    #[test]
    fn test_extract_portal_url_with_path_and_query() {
        let html =
            r#"window.location="https://login.example.com:8443/portal/login?sessionid=abc123""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("https://login.example.com:8443/portal/login?sessionid=abc123".to_string())
        );
    }

    #[test]
    fn test_extract_portal_url_http_protocol() {
        let html = r#"window.location="http://portal.insecure.com""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("http://portal.insecure.com".to_string())
        );
    }

    #[test]
    fn test_extract_magic_value_case_sensitive() {
        // "magic" should be lowercase to match
        let html = r#"<input type="hidden" name="MAGIC" value="uppercase_magic">"#;
        // This should not match because the regex looks for lowercase "magic"
        assert_eq!(extract_magic_value(html), None);
    }

    #[test]
    fn test_extract_magic_value_single_quotes() {
        // Test with single quotes instead of double quotes (should not match)
        let html = r#"<input type='hidden' name='magic' value='single_quote_magic'>"#;
        // The regex specifically looks for double quotes
        assert_eq!(extract_magic_value(html), None);
    }

    #[test]
    fn test_extract_portal_url_with_fragment() {
        let html = r#"window.location="https://portal.test.com/login#section""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("https://portal.test.com/login#section".to_string())
        );
    }

    #[test]
    fn test_extract_portal_url_malformed_no_quotes() {
        let html = r#"window.location=https://portal.test.com"#;
        assert_eq!(extract_captive_portal_url(html), None);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_LOGIN_RETRIES, 3);
        assert_eq!(INITIAL_RETRY_DELAY_SECS, 2);
        assert_eq!(LOGOUT_URL, "https://login.iitmandi.ac.in:1003/logout?");
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(10));
        assert_eq!(
            DEFAULT_CONNECTIVITY_CHECK_URL,
            "http://clients3.google.com/generate_204"
        );
    }

    #[test]
    fn test_get_connectivity_check_url_default() {
        // Test that default URL is returned when env var is not set
        std::env::remove_var("ACP_CONNECTIVITY_URL");
        let url = get_connectivity_check_url();
        assert_eq!(url, DEFAULT_CONNECTIVITY_CHECK_URL);
    }

    #[test]
    fn test_portal_url_regex_compilation() {
        // Test that the regex compiles and can be used
        let regex = portal_url_regex();
        assert!(regex
            .is_match(r#"window.location="https://example.com""#));
    }

    #[test]
    fn test_magic_value_regex_compilation() {
        // Test that the regex compiles and can be used
        let regex = magic_value_regex();
        assert!(regex.is_match(r#"<input type="hidden" name="magic" value="test">"#));
    }

    #[test]
    fn test_extract_magic_value_long_value() {
        // Test with a very long magic value
        let long_value = "a".repeat(1000);
        let html = format!(
            r#"<input type="hidden" name="magic" value="{}">"#,
            long_value
        );
        assert_eq!(extract_magic_value(&html), Some(long_value));
    }

    #[test]
    fn test_extract_portal_url_long_url() {
        // Test with a very long URL
        let long_url = format!("https://portal.example.com/{}", "path/".repeat(100));
        let html = format!(r#"window.location="{}""#, long_url);
        assert_eq!(extract_captive_portal_url(&html), Some(long_url));
    }

    #[test]
    fn test_extract_magic_value_with_encoded_characters() {
        // Test with URL-encoded characters
        let html = r#"<input type="hidden" name="magic" value="test%20value%2Bencoded">"#;
        assert_eq!(
            extract_magic_value(html),
            Some("test%20value%2Bencoded".to_string())
        );
    }

    #[test]
    fn test_extract_portal_url_with_ipv4() {
        let html = r#"window.location="http://192.168.1.1/portal""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("http://192.168.1.1/portal".to_string())
        );
    }

    #[test]
    fn test_extract_portal_url_with_ipv6() {
        let html = r#"window.location="http://[2001:db8::1]/portal""#;
        assert_eq!(
            extract_captive_portal_url(html),
            Some("http://[2001:db8::1]/portal".to_string())
        );
    }

    #[test]
    fn test_max_login_retries_positive() {
        assert!(MAX_LOGIN_RETRIES > 0);
    }

    #[test]
    fn test_initial_retry_delay_positive() {
        assert!(INITIAL_RETRY_DELAY_SECS > 0);
    }

    #[test]
    fn test_request_timeout_reasonable() {
        assert!(REQUEST_TIMEOUT.as_secs() >= 5);
        assert!(REQUEST_TIMEOUT.as_secs() <= 60);
    }
}