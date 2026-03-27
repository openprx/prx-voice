//! Phase 4 integration tests: multi-channel, handoff, recording.

use prx_voice_adapter::mock_agent::{MockAgent, MockAgentConfig};
use prx_voice_adapter::mock_asr::{MockAsr, MockAsrConfig};
use prx_voice_adapter::mock_tts::{MockTts, MockTtsConfig};
use prx_voice_event::bus::{EventBus, EventBusConfig};
use prx_voice_observe::metrics::MetricsRegistry;
use prx_voice_session::config::SessionConfig;
use prx_voice_session::handoff::*;
use prx_voice_session::recording::*;
use prx_voice_transport::channel::*;
use prx_voice_transport::mock::*;
use prx_voice_types::ids::*;
use std::sync::Arc;

#[tokio::test]
async fn transport_to_session_flow() {
    // Create a mock transport with preloaded audio
    let frames = vec![
        AudioFrame {
            data: vec![0u8; 320],
            sample_rate: 16000,
            channels: 1,
            encoding: AudioEncoding::Pcm16,
            timestamp_ms: 0,
            sequence: 0,
        },
        AudioFrame {
            data: vec![0u8; 320],
            sample_rate: 16000,
            channels: 1,
            encoding: AudioEncoding::Pcm16,
            timestamp_ms: 20,
            sequence: 1,
        },
    ];

    let mut transport = MockTransport::new(MockTransportConfig {
        channel_type: ChannelType::WebConsole,
        direction: Direction::Inbound,
        preloaded_frames: frames,
        ..Default::default()
    });

    // Open transport
    let (mut audio_rx, _audio_tx) = transport.open().await.unwrap();
    assert!(transport.is_connected());
    assert_eq!(transport.channel_type(), ChannelType::WebConsole);

    // Receive audio frames
    let f1 = audio_rx.recv().await.unwrap();
    assert_eq!(f1.sequence, 0);
    let f2 = audio_rx.recv().await.unwrap();
    assert_eq!(f2.sequence, 1);

    // Create a session using the transport's audio
    let bus = EventBus::new(EventBusConfig::default());
    let asr = Box::new(MockAsr::new(MockAsrConfig {
        latency_ms: 5,
        ..Default::default()
    }));
    let agent = Box::new(MockAgent::new(MockAgentConfig {
        first_token_latency_ms: 5,
        ..Default::default()
    }));
    let tts = Box::new(MockTts::new(MockTtsConfig {
        first_chunk_latency_ms: 5,
        ..Default::default()
    }));

    let mut orch = prx_voice_session::orchestrator::SessionOrchestrator::new(
        TenantId::new(),
        SessionConfig::default(),
        asr,
        agent,
        tts,
        bus,
        Arc::new(MetricsRegistry::new()),
    );
    orch.start().await.unwrap();

    // Execute turn with audio data from transport
    let response = orch.execute_turn(Some(f1.data)).await.unwrap();
    assert!(!response.is_empty());

    transport.close().await.unwrap();
    orch.close("test").await.unwrap();
}

#[test]
fn handoff_full_lifecycle_with_recording() {
    let handoff_mgr = HandoffManager::new();
    let recording_store = RecordingStore::new();

    let sid = SessionId::new();
    let tid = TenantId::new();

    // Start recording when session begins
    let rec = recording_store.start_recording(
        sid,
        tid,
        StreamRole::Mixed,
        "pcm16",
        16000,
        RetentionClass::StandardOperational,
    );
    assert_eq!(rec.status, RecordingStatus::Active);

    // Session requests human handoff
    let handoff = handoff_mgr.create_request(
        sid,
        tid,
        HandoffTarget::SpecificQueue {
            queue_id: "support-q1".into(),
        },
        "User requested agent",
        Some("User asked about billing dispute".into()),
    );
    assert_eq!(handoff.status, HandoffStatus::Pending);

    // Queue assignment
    handoff_mgr
        .update_status(handoff.handoff_id, HandoffStatus::Queued)
        .unwrap();
    handoff_mgr
        .set_queue_position(handoff.handoff_id, 2, 60)
        .unwrap();

    // Agent picks up
    handoff_mgr
        .assign_agent(handoff.handoff_id, "agent-sarah")
        .unwrap();
    let updated = handoff_mgr.get(handoff.handoff_id).unwrap();
    assert_eq!(updated.status, HandoffStatus::Assigned);

    // Agent confirms handoff
    handoff_mgr
        .update_status(handoff.handoff_id, HandoffStatus::Confirmed)
        .unwrap();

    // Complete recording after session ends
    let completed_rec = recording_store
        .complete_recording(
            rec.recording_id,
            125000,
            Some("s3://prx-voice/recordings/session-123.wav".into()),
        )
        .unwrap();
    assert_eq!(completed_rec.status, RecordingStatus::Completed);
    assert_eq!(completed_rec.duration_ms, 125000);

    // Verify session's recordings
    let session_recs = recording_store.get_by_session(sid);
    assert_eq!(session_recs.len(), 1);
    assert_eq!(session_recs[0].stream_role, StreamRole::Mixed);
}

#[test]
fn multi_channel_transport_types() {
    // Verify all channel types are distinct and serialize correctly
    let types = [
        ChannelType::WebConsole,
        ChannelType::WebSocket,
        ChannelType::Sip,
        ChannelType::WebRtc,
        ChannelType::AppSdk,
    ];

    for (i, ct) in types.iter().enumerate() {
        for (j, other) in types.iter().enumerate() {
            if i == j {
                assert_eq!(ct, other);
            } else {
                assert_ne!(ct, other);
            }
        }
        // Verify serialization roundtrip
        let json = serde_json::to_string(ct).unwrap();
        let back: ChannelType = serde_json::from_str(&json).unwrap();
        assert_eq!(*ct, back);
    }
}
