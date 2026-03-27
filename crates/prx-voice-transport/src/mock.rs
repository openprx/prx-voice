//! Mock transport channel for testing.

use crate::channel::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// Mock transport configuration.
#[derive(Debug, Clone)]
pub struct MockTransportConfig {
    pub channel_type: ChannelType,
    pub direction: Direction,
    pub sample_rate: u32,
    pub encoding: AudioEncoding,
    /// Pre-loaded audio frames to deliver on open.
    pub preloaded_frames: Vec<AudioFrame>,
}

impl Default for MockTransportConfig {
    fn default() -> Self {
        Self {
            channel_type: ChannelType::WebConsole,
            direction: Direction::Inbound,
            sample_rate: 16000,
            encoding: AudioEncoding::Pcm16,
            preloaded_frames: Vec::new(),
        }
    }
}

/// Mock transport channel.
pub struct MockTransport {
    config: MockTransportConfig,
    connected: Arc<AtomicBool>,
    info: ConnectionInfo,
}

impl MockTransport {
    pub fn new(config: MockTransportConfig) -> Self {
        let info = ConnectionInfo {
            channel_type: config.channel_type,
            direction: config.direction,
            remote_addr: Some("127.0.0.1:0".into()),
            from_uri: None,
            to_uri: None,
            codec: config.encoding,
            sample_rate: config.sample_rate,
        };
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            info,
        }
    }
}

#[async_trait::async_trait]
impl TransportChannel for MockTransport {
    async fn open(
        &mut self,
    ) -> Result<(mpsc::Receiver<AudioFrame>, mpsc::Sender<AudioFrame>), TransportError> {
        self.connected.store(true, Ordering::Relaxed);

        let (ingress_tx, ingress_rx) = mpsc::channel(64);
        let (egress_tx, _egress_rx) = mpsc::channel(64);

        // Deliver preloaded frames
        let frames = self.config.preloaded_frames.clone();
        tokio::spawn(async move {
            for frame in frames {
                if ingress_tx.send(frame).await.is_err() {
                    break;
                }
            }
        });

        Ok((ingress_rx, egress_tx))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn connection_info(&self) -> &ConnectionInfo {
        &self.info
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn channel_type(&self) -> ChannelType {
        self.config.channel_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_transport_opens_and_closes() {
        let mut transport = MockTransport::new(MockTransportConfig::default());
        assert!(!transport.is_connected());

        let (_rx, _tx) = transport.open().await.unwrap();
        assert!(transport.is_connected());
        assert_eq!(transport.channel_type(), ChannelType::WebConsole);

        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[tokio::test]
    async fn mock_transport_delivers_preloaded_frames() {
        let frames = vec![
            AudioFrame {
                data: vec![1, 2, 3],
                sample_rate: 16000,
                channels: 1,
                encoding: AudioEncoding::Pcm16,
                timestamp_ms: 0,
                sequence: 0,
            },
            AudioFrame {
                data: vec![4, 5, 6],
                sample_rate: 16000,
                channels: 1,
                encoding: AudioEncoding::Pcm16,
                timestamp_ms: 20,
                sequence: 1,
            },
        ];
        let mut transport = MockTransport::new(MockTransportConfig {
            preloaded_frames: frames,
            ..Default::default()
        });

        let (mut rx, _tx) = transport.open().await.unwrap();

        let f1 = rx.recv().await.unwrap();
        assert_eq!(f1.data, vec![1, 2, 3]);
        assert_eq!(f1.sequence, 0);

        let f2 = rx.recv().await.unwrap();
        assert_eq!(f2.data, vec![4, 5, 6]);
        assert_eq!(f2.sequence, 1);
    }

    #[test]
    fn channel_type_display() {
        assert_eq!(ChannelType::Sip.to_string(), "sip");
        assert_eq!(ChannelType::WebRtc.to_string(), "webrtc");
        assert_eq!(ChannelType::WebConsole.to_string(), "web_console");
    }

    #[test]
    fn connection_info_serializes() {
        let info = ConnectionInfo {
            channel_type: ChannelType::Sip,
            direction: Direction::Inbound,
            remote_addr: Some("10.0.0.1:5060".into()),
            from_uri: Some("sip:alice@example.com".into()),
            to_uri: Some("sip:bob@example.com".into()),
            codec: AudioEncoding::Mulaw,
            sample_rate: 8000,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("sip"));
        assert!(json.contains("mulaw"));
        assert!(json.contains("alice@example.com"));
    }
}
