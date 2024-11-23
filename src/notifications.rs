use notify_rust::Notification;

pub async fn send_notification(message: &str) {
    Notification::new()
        .body(message)
        .appname("Auto Captive Portal")
        .timeout(5)
        .show()
        .ok();
}
