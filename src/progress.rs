use rmcp::RoleServer;
use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::Peer;

#[derive(Clone)]
pub struct ProgressSender {
    peer: Peer<RoleServer>,
    token: ProgressToken,
}

impl ProgressSender {
    pub fn new(peer: Peer<RoleServer>, token: ProgressToken) -> Self {
        Self { peer, token }
    }

    pub fn send(&self, progress: f64, total: Option<f64>, message: Option<String>) {
        let mut params = ProgressNotificationParam::new(self.token.clone(), progress);
        if let Some(total) = total {
            params = params.with_total(total);
        }
        if let Some(message) = message {
            params = params.with_message(message);
        }
        let peer = self.peer.clone();
        tokio::spawn(async move {
            let _ = peer.notify_progress(params).await;
        });
    }
}

tokio::task_local! {
    pub static PROGRESS_SENDER: Option<ProgressSender>;
}

#[cfg(test)]
mod tests {
    use super::PROGRESS_SENDER;

    // Guards against the bug this replaced: a `Mutex<Option<ProgressSender>>`
    // field on the server that a prior call's sender could linger in and
    // leak into a later, unrelated call. A task-local only exists inside its
    // own `.scope()`, so nothing is visible before or after it.
    #[tokio::test]
    async fn no_ambient_sender_outside_any_scope() {
        assert!(PROGRESS_SENDER.try_with(|_| ()).is_err());
    }

    #[tokio::test]
    async fn scoped_value_does_not_leak_to_the_next_call() {
        let seen_inside = PROGRESS_SENDER
            .scope(None, async {
                PROGRESS_SENDER.try_with(|s| s.is_none()).unwrap()
            })
            .await;
        assert!(seen_inside);
        assert!(PROGRESS_SENDER.try_with(|_| ()).is_err());
    }
}
