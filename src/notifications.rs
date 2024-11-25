use notify_rust::Notification;

pub async fn send_notification(message: &str) {
    Notification::new()
        .summary("Auto Captive Portal")
        .body(message)
        .appname("Auto Captive Portal")
        .timeout(5)
        .show()
        .ok();
}
