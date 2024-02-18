use dotenv::dotenv;

mod captive_portal {
    use headless_chrome::Browser;
    use regex::Regex;
    use reqwest::StatusCode;

    pub async fn login(
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let browser = Browser::default().expect("failed to launch browser");

        let tab = browser.new_tab().expect("failed to open tab");
        tab.navigate_to(url).expect("failed to navigate");
        tab.wait_until_navigated()
            .expect("failed to wait until navigated");

        let username_selector = "#ft_un";
        let password_selector = "#ft_pd";

        let username_field = tab
            .find_element(username_selector)
            .expect("failed to find username");
        username_field.click().expect("failed to click username");
        username_field
            .type_into(username)
            .expect("failed to type username");

        let password_field = tab
            .find_element(password_selector)
            .expect("failed to find password");
        password_field.click().expect("failed to click password");
        password_field
            .type_into(password)
            .expect("failed to type password");

        let submit_selector = "button[type='submit']";
        let submit_button = tab
            .find_element(submit_selector)
            .expect("failed to find submit button");
        submit_button
            .click()
            .expect("failed to click submit button");
        tab.wait_until_navigated()
            .expect("failed to wait until navigated");

        // Wait for 2 seconds for the page to load
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // let png_data =
        //     tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)?;
        // std::fs::write("screenshot.png", png_data)?;

        // check if login was successful by checking text 'Authentication Successful' on page
        let success_selector = "h2";
        let success_element = tab
            .find_element(success_selector)
            .expect("failed to find success element");
        let success_text: String = success_element
            .get_content()
            .expect("failed to get success text");
        if success_text.contains("Authentication Successful") {
            println!("login successful");
        } else {
            println!("login failed");
        }

        Ok(())
    }

    pub fn extract_captive_portal_url(html: &str) -> Option<String> {
        let re = Regex::new(r#"window\.location="([^"]*)""#).unwrap();
        re.captures(html)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    pub async fn check_captive_portal() -> Result<Option<String>, reqwest::Error> {
        let url = "http://clients3.google.com/generate_204";
        let client = reqwest::Client::new();
        let resp = client.get(url).send().await.expect("request failed");

        match resp.status() {
            StatusCode::NO_CONTENT => Ok(None),
            StatusCode::OK => {
                let html = resp.text().await.expect("failed to get response body");
                let captive_portal_url = extract_captive_portal_url(&html);

                if let Some(url) = captive_portal_url {
                    Ok(Some(url))
                } else {
                    Ok(None)
                }
            }
            _ => Err(resp.error_for_status().unwrap_err()),
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let username = std::env::var("LDAP_USERNAME").expect("LDAP_USERNAME not set");
    let password = std::env::var("LDAP_PASSWORD").expect("LDAP_PASSWORD not set");
    loop {
        match captive_portal::check_captive_portal().await {
            Ok(Some(url)) => {
                println!("Captive portal detected at {}", url);
                match captive_portal::login(&url, &username, &password).await {
                    Ok(_) => {
                        println!("Successfully logged in");
                        std::process::Command::new("notify-send")
                            .arg("Captive portal detected and logged in successfully")
                            .output()
                            .expect("Failed to send notification");
                    }
                    Err(e) => {
                        eprintln!("Failed to log in: {}", e);
                        std::process::Command::new("systemctl")
                            .arg("--user")
                            .arg("restart")
                            .arg("acp")
                            .output()
                            .expect("Failed to restart service");
                    }
                }
            }
            Ok(None) => {
                println!("No captive portal detected");
            }
            Err(e) => {
                eprintln!("Failed to check for captive portal: {}", e);
                std::process::Command::new("systemctl")
                    .arg("--user")
                    .arg("restart")
                    .arg("acp")
                    .output()
                    .expect("Failed to restart service");
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}
