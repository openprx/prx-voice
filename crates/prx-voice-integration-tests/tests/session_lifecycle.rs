//! Integration test: full session lifecycle with mock adapters.

use prx_voice_adapter::mock_agent::{MockAgent, MockAgentConfig};
use prx_voice_adapter::mock_asr::{MockAsr, MockAsrConfig};
use prx_voice_adapter::mock_tts::{MockTts, MockTtsConfig};
use prx_voice_event::bus::{EventBus, EventBusConfig};
use prx_voice_event::payload::event_types;
use prx_voice_observe::metrics::MetricsRegistry;
use prx_voice_session::config::SessionConfig;
use prx_voice_session::orchestrator::SessionOrchestrator;
use prx_voice_state::transition::SessionState;
use prx_voice_types::ids::TenantId;
use std::sync::Arc;

fn make_orchestrator(event_bus: EventBus) -> SessionOrchestrator {
    let config = SessionConfig::default();
    let asr = Box::new(MockAsr::new(MockAsrConfig {
        latency_ms: 10,
        transcript: "I want to check my balance".into(),
        ..Default::default()
    }));
    let agent = Box::new(MockAgent::new(MockAgentConfig {
        first_token_latency_ms: 10,
        response_text: "Your balance is $42.15. Anything else?".into(),
        ..Default::default()
    }));
    let tts = Box::new(MockTts::new(MockTtsConfig {
        first_chunk_latency_ms: 10,
        ..Default::default()
    }));

    SessionOrchestrator::new(
        TenantId::new(),
        config,
        asr,
        agent,
        tts,
        event_bus,
        Arc::new(MetricsRegistry::new()),
    )
}

#[tokio::test]
async fn test_complete_3_turn_session() {
    let event_bus = EventBus::new(EventBusConfig::default());
    let mut sub = event_bus.subscribe();
    let mut orch = make_orchestrator(event_bus);

    // Start
    orch.start().await.unwrap();
    assert_eq!(orch.state(), SessionState::Listening);

    // Turn 1
    let r1 = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
    assert!(r1.contains("balance"));
    assert_eq!(orch.state(), SessionState::Listening);

    // Turn 2
    let r2 = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
    assert!(!r2.is_empty());

    // Turn 3
    let r3 = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
    assert!(!r3.is_empty());
    assert_eq!(orch.current_turn().as_u32(), 4); // 3 turns completed

    // Close
    orch.close("normal_clearing").await.unwrap();
    assert_eq!(orch.state(), SessionState::Closed);

    // Verify events were emitted (collect available events)
    let mut event_types_seen = vec![];
    // Drain available events with a timeout
    while let Ok(Some(evt)) =
        tokio::time::timeout(tokio::time::Duration::from_millis(100), sub.recv()).await
    {
        event_types_seen.push(evt.event_type.clone());
    }

    assert!(
        event_types_seen.contains(&event_types::SESSION_CREATED.to_string()),
        "Missing SESSION_CREATED event"
    );
    assert!(
        event_types_seen.contains(&event_types::SESSION_CLOSED.to_string()),
        "Missing SESSION_CLOSED event"
    );
    assert!(
        event_types_seen.contains(&event_types::TRANSCRIPT_FINAL.to_string()),
        "Missing TRANSCRIPT_FINAL event"
    );
}
