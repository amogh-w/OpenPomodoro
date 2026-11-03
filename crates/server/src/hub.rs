//! Live updates: tells a user's open WebSockets that something changed, so other tabs and devices refetch.
//! The message carries no data, only "something changed", so a missed message cannot leave a client wrong for
//! long: any later message, or a reconnect, makes it fetch the truth.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::broadcast;

pub const CHANGED: &str = r#"{"type":"changed"}"#;
/// Most WebSockets one user may keep open at once (a handful of tabs and devices).
pub const MAX_CONNECTIONS_PER_USER: usize = 10;
const CHANNEL_CAPACITY: usize = 16;

struct Entry {
    tx: broadcast::Sender<String>,
    connections: usize,
}

#[derive(Default)]
pub struct Hub {
    users: Mutex<HashMap<String, Entry>>,
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.users
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Register one more connection for `user`. `None` when they already have the maximum.
    pub fn subscribe(&self, user: &str) -> Option<broadcast::Receiver<String>> {
        let mut users = self.lock();
        let entry = users.entry(user.to_string()).or_insert_with(|| Entry {
            tx: broadcast::channel(CHANNEL_CAPACITY).0,
            connections: 0,
        });
        if entry.connections >= MAX_CONNECTIONS_PER_USER {
            return None;
        }
        entry.connections += 1;
        Some(entry.tx.subscribe())
    }

    /// A connection ended. The user's channel is dropped with the last one.
    pub fn release(&self, user: &str) {
        let mut users = self.lock();
        if let Some(entry) = users.get_mut(user) {
            entry.connections = entry.connections.saturating_sub(1);
            if entry.connections == 0 {
                users.remove(user);
            }
        }
    }

    /// Tell every open connection of `user` that their data changed. Nobody else hears it.
    pub fn publish(&self, user: &str, message: &str) {
        if let Some(entry) = self.lock().get(user) {
            // No receivers is fine: nobody is listening right now.
            let _ = entry.tx.send(message.to_string());
        }
    }

    pub fn connections(&self, user: &str) -> usize {
        self.lock().get(user).map_or(0, |e| e.connections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_reaches_only_that_users_connections() {
        let hub = Hub::default();
        let mut a1 = hub.subscribe("alice").expect("a1");
        let mut a2 = hub.subscribe("alice").expect("a2");
        let mut b = hub.subscribe("bob").expect("b");
        hub.publish("alice", CHANGED);
        assert_eq!(a1.recv().await.expect("a1"), CHANGED);
        assert_eq!(a2.recv().await.expect("a2"), CHANGED);
        assert!(b.try_recv().is_err(), "Bob must not hear Alice's changes");
    }

    #[test]
    fn publishing_with_nobody_listening_is_harmless() {
        let hub = Hub::default();
        hub.publish("nobody", CHANGED);
        assert_eq!(hub.connections("nobody"), 0);
    }

    #[test]
    fn connections_are_capped_per_user_and_released() {
        let hub = Hub::default();
        let held: Vec<_> = (0..MAX_CONNECTIONS_PER_USER)
            .map(|_| hub.subscribe("alice").expect("slot"))
            .collect();
        assert!(hub.subscribe("alice").is_none(), "one more than the cap");
        assert!(hub.subscribe("bob").is_some(), "the cap is per user");
        hub.release("alice");
        assert!(
            hub.subscribe("alice").is_some(),
            "a freed slot can be reused"
        );
        drop(held);
    }

    #[test]
    fn the_channel_is_removed_with_the_last_connection() {
        let hub = Hub::default();
        let _rx = hub.subscribe("alice").expect("slot");
        hub.release("alice");
        assert_eq!(hub.connections("alice"), 0);
        assert!(hub.lock().is_empty());
    }
}
