use log::{error, info};
use notify_rust::Notification;

pub async fn send_notification(message: &str) {
    info!("Sending notification: {message}");

    let message = message.to_string();

    let result = tokio::task::spawn_blocking(move || {
        Notification::new()
            .summary("Auto Captive Portal")
            .body(&message)
            .appname("Auto Captive Portal")
            .icon("dialog-information")
            .timeout(5000)
            .show()
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            info!("Notification sent successfully");
        }
        Ok(Err(e)) => {
            error!("Failed to send notification: {e}");
        }
        Err(e) => {
            error!("Failed to spawn notification task: {e}");
        }
    }
}
