use crate::error::{AppError, Result};
use headless_chrome::Browser;
use log::{error, info};
use regex::Regex;
use reqwest::StatusCode;

pub async fn login(url: &str, username: &str, password: &str) -> Result<()> {
    let browser: Browser = Browser::default().map_err(|e| AppError::Browser(e.to_string()))?;

    let tab: std::sync::Arc<headless_chrome::Tab> = browser
        .new_tab()
        .map_err(|e| AppError::Browser(e.to_string()))?;
    tab.navigate_to(url)
        .map_err(|e| AppError::Browser(e.to_string()))?;
    tab.wait_until_navigated()
        .map_err(|e| AppError::Browser(e.to_string()))?;

    let username_selector: &str = "#ft_un";
    let password_selector: &str = "#ft_pd";

    let username_field: headless_chrome::Element<'_> = tab
        .find_element(username_selector)
        .map_err(|e| AppError::Browser(e.to_string()))?;
    username_field
        .click()
        .map_err(|e| AppError::Browser(e.to_string()))?;
    username_field
        .type_into(username)
        .map_err(|e| AppError::Browser(e.to_string()))?;

    let password_field: headless_chrome::Element<'_> = tab
        .find_element(password_selector)
        .map_err(|e| AppError::Browser(e.to_string()))?;
    password_field
        .click()
        .map_err(|e| AppError::Browser(e.to_string()))?;
    password_field
        .type_into(password)
        .map_err(|e| AppError::Browser(e.to_string()))?;

    let submit_selector: &str = "button[type='submit']";
    let submit_button: headless_chrome::Element<'_> = tab
        .find_element(submit_selector)
        .map_err(|e| AppError::Browser(e.to_string()))?;
    submit_button
        .click()
        .map_err(|e| AppError::Browser(e.to_string()))?;
    tab.wait_until_navigated()
        .map_err(|e| AppError::Browser(e.to_string()))?;

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let success_selector: &str = "h2";
    let success_element: headless_chrome::Element<'_> = tab
        .find_element(success_selector)
        .map_err(|e| AppError::Browser(e.to_string()))?;
    let success_text: String = success_element
        .get_content()
        .map_err(|e| AppError::Browser(e.to_string()))?;

    if success_text.contains("Authentication Successful") {
        info!("Successfully authenticated with the captive portal.");
        Ok(())
    } else {
        error!("Authentication unsuccessful.");
        Err(AppError::LoginFailed(
            "Authentication unsuccessful".to_string(),
        ))
    }
}

pub fn extract_captive_portal_url(html: &str) -> Option<String> {
    let re: Regex = Regex::new(r#"window\.location="([^"]*)""#).unwrap();
    re.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

pub async fn check_captive_portal() -> Result<Option<String>> {
    let url: &str = "http://clients3.google.com/generate_204";
    let client: reqwest::Client = reqwest::Client::new();
    let resp: reqwest::Response = client.get(url).send().await?;

    match resp.status() {
        StatusCode::NO_CONTENT => Ok(None),
        StatusCode::OK => {
            let html: String = resp.text().await?;
            let captive_portal_url: Option<String> = extract_captive_portal_url(&html);

            if let Some(url) = captive_portal_url {
                Ok(Some(url))
            } else {
                Ok(None)
            }
        }
        _ => Err(AppError::Network(resp.error_for_status().unwrap_err())),
    }
}
