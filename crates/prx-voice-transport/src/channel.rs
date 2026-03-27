//! Transport channel abstraction.
//!
//! Defines a unified interface for receiving audio from and sending audio to
//! clients, regardless of underlying transport (WebSocket, SIP, WebRTC).

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

/// Transport channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    WebConsole,
    WebSocket,
    Sip,
    WebRtc,
    AppSdk,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WebConsole => write!(f, "web_console"),
            Self::WebSocket => write!(f, "websocket"),
            Self::Sip => write!(f, "sip"),
            Self::WebRtc => write!(f, "webrtc"),
            Self::AppSdk => write!(f, "app_sdk"),
        }
    }
}

/// Call direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Inbound,
    Outbound,
}

/// Audio frame from/to client.
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub encoding: AudioEncoding,
    pub timestamp_ms: u64,
    pub sequence: u64,
}

/// Audio encoding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioEncoding {
    Pcm16,
    Opus,
    Mulaw,
    Alaw,
}

/// Transport errors.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Transport timeout")]
    Timeout,
    #[error("Unsupported encoding: {0:?}")]
    UnsupportedEncoding(AudioEncoding),
}

/// Transport connection metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub channel_type: ChannelType,
    pub direction: Direction,
    pub remote_addr: Option<String>,
    pub from_uri: Option<String>,
    pub to_uri: Option<String>,
    pub codec: AudioEncoding,
    pub sample_rate: u32,
}

/// The transport channel trait.
/// Implementations handle the specifics of each transport protocol.
#[async_trait::async_trait]
pub trait TransportChannel: Send + Sync {
    /// Open the transport connection.
    /// Returns (audio_ingress_rx, audio_egress_tx).
    async fn open(
        &mut self,
    ) -> Result<(mpsc::Receiver<AudioFrame>, mpsc::Sender<AudioFrame>), TransportError>;

    /// Close the transport connection.
    async fn close(&mut self) -> Result<(), TransportError>;

    /// Connection metadata.
    fn connection_info(&self) -> &ConnectionInfo;

    /// Whether the transport is still connected.
    fn is_connected(&self) -> bool;

    /// Channel type.
    fn channel_type(&self) -> ChannelType;
}
