//! In-memory event bus for single-machine Phase 1.
//!
//! Uses tokio broadcast channel. Subscribers receive events
//! with at-least-once semantics (lagged subscribers may miss events).

use crate::envelope::VoiceEvent;
use tokio::sync::broadcast;
use tracing::warn;

/// Configuration for the event bus.
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Channel capacity.
    pub capacity: usize,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self { capacity: 1024 }
    }
}

/// In-memory broadcast event bus.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<VoiceEvent>,
}

impl EventBus {
    /// Create a new event bus.
    pub fn new(config: EventBusConfig) -> Self {
        let (sender, _) = broadcast::channel(config.capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    /// Returns the number of receivers that received the event.
    pub fn publish(&self, event: VoiceEvent) -> usize {
        self.sender.send(event).unwrap_or_default()
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber {
            receiver: self.sender.subscribe(),
        }
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// An event subscriber.
pub struct EventSubscriber {
    receiver: broadcast::Receiver<VoiceEvent>,
}

impl EventSubscriber {
    /// Receive the next event. Returns None if the bus is closed.
    pub async fn recv(&mut self) -> Option<VoiceEvent> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => return Some(event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(missed = n, "event subscriber lagged, missed events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Severity;
    use prx_voice_types::ids::*;

    fn test_event(seq: u64) -> VoiceEvent {
        VoiceEvent::new(
            "prx-voice/test",
            "prx.voice.test.ping",
            TenantId::new(),
            SessionId::new(),
            TurnId::first(),
            seq,
            TraceId::new(),
            Severity::Info,
            serde_json::json!({"seq": seq}),
        )
    }

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = EventBus::new(EventBusConfig::default());
        let mut sub = bus.subscribe();

        bus.publish(test_event(1));
        bus.publish(test_event(2));

        let e1 = sub.recv().await.unwrap();
        assert_eq!(e1.prx_seq, 1);

        let e2 = sub.recv().await.unwrap();
        assert_eq!(e2.prx_seq, 2);
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let bus = EventBus::new(EventBusConfig::default());
        let mut sub1 = bus.subscribe();
        let mut sub2 = bus.subscribe();

        bus.publish(test_event(1));

        let e1 = sub1.recv().await.unwrap();
        let e2 = sub2.recv().await.unwrap();
        assert_eq!(e1.prx_seq, 1);
        assert_eq!(e2.prx_seq, 1);
    }

    #[test]
    fn publish_without_subscribers_returns_zero() {
        let bus = EventBus::new(EventBusConfig::default());
        let count = bus.publish(test_event(1));
        assert_eq!(count, 0);
    }
}
