use log::{error, info};
use notify_rust::Notification;

pub async fn send_notification(message: &str) {
    info!("Sending notification: {message}");

    let result = Notification::new()
        .summary("Auto Captive Portal")
        .body(message)
        .appname("Auto Captive Portal")
        .icon("dialog-information")
        .timeout(5000)
        .show();

    match result {
        Ok(_) => {
            info!("Notification sent successfully");
        }
        Err(e) => {
            error!("Failed to send notification: {e}");
        }
    }
}
