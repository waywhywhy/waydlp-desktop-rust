//! Desktop notifications (sent from a background thread so the UI never blocks).

pub fn send(summary: String, body: String) {
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.summary(&summary).body(&body).appname("waydlp");
        #[cfg(target_os = "linux")]
        n.icon("io.github.waywhywhy.waydlp");
        if let Err(e) = n.show() {
            tracing::debug!(error = %e, "desktop notification failed");
        }
    });
}
