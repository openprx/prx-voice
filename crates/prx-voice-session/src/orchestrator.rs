//! Session orchestrator — the actor that owns a single session.

use crate::config::SessionConfig;
use prx_voice_adapter::agent::{AgentAdapter, AgentContext, ConversationTurn};
use prx_voice_adapter::asr::AsrAdapter;
use prx_voice_adapter::tts::{TtsAdapter, TtsSynthesisRequest};
use prx_voice_event::bus::EventBus;
use prx_voice_event::envelope::{Severity, VoiceEvent};
use prx_voice_event::payload::event_types;
use prx_voice_observe::metrics::{MetricsRegistry, metric_names};
use prx_voice_state::machine::SessionStateMachine;
use prx_voice_state::transition::{SessionState, TransitionResult, Trigger};
use prx_voice_types::ids::*;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Session orchestrator errors.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Session already closed")]
    AlreadyClosed,
    #[error("Adapter initialization failed: {0}")]
    AdapterInitFailed(String),
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// The session orchestrator actor.
pub struct SessionOrchestrator {
    // Identity
    session_id: SessionId,
    tenant_id: TenantId,
    trace_id: TraceId,

    // Config
    config: SessionConfig,

    // State
    state_machine: SessionStateMachine,
    current_turn: TurnId,
    seq_counter: u64,
    conversation_history: Vec<ConversationTurn>,

    // Adapters
    asr: Arc<Mutex<Box<dyn AsrAdapter>>>,
    agent: Arc<Mutex<Box<dyn AgentAdapter>>>,
    tts: Arc<Mutex<Box<dyn TtsAdapter>>>,

    // Event bus
    event_bus: EventBus,

    // Metrics
    metrics: Arc<MetricsRegistry>,
}

impl SessionOrchestrator {
    /// Create a new session orchestrator.
    pub fn new(
        tenant_id: TenantId,
        config: SessionConfig,
        asr: Box<dyn AsrAdapter>,
        agent: Box<dyn AgentAdapter>,
        tts: Box<dyn TtsAdapter>,
        event_bus: EventBus,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            session_id: SessionId::new(),
            tenant_id,
            trace_id: TraceId::new(),
            state_machine: SessionStateMachine::new(config.state_machine.clone()),
            current_turn: TurnId::first(),
            seq_counter: 0,
            conversation_history: Vec::new(),
            config,
            asr: Arc::new(Mutex::new(asr)),
            agent: Arc::new(Mutex::new(agent)),
            tts: Arc::new(Mutex::new(tts)),
            event_bus,
            metrics,
        }
    }

    /// Session ID.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Current state.
    pub fn state(&self) -> SessionState {
        self.state_machine.state()
    }

    /// Current turn.
    pub fn current_turn(&self) -> TurnId {
        self.current_turn
    }

    fn next_seq(&mut self) -> u64 {
        self.seq_counter += 1;
        self.seq_counter
    }

    fn emit_event(&mut self, event_type: &str, severity: Severity, data: serde_json::Value) {
        let seq = self.next_seq();
        let event = VoiceEvent::new(
            "prx-voice/orchestrator",
            event_type,
            self.tenant_id,
            self.session_id,
            self.current_turn,
            seq,
            self.trace_id,
            severity,
            data,
        );
        self.event_bus.publish(event);
    }

    fn apply_trigger(&mut self, trigger: Trigger) -> Result<SessionState, OrchestratorError> {
        let result = self.state_machine.apply(trigger.clone());
        match result {
            TransitionResult::Transitioned { from, to, .. } => {
                self.emit_event(
                    event_types::SESSION_STATE_CHANGED,
                    Severity::Info,
                    json!({
                        "previous_state": from.to_string(),
                        "new_state": to.to_string(),
                        "trigger_reason": format!("{trigger:?}"),
                    }),
                );
                Ok(to)
            }
            TransitionResult::Rejected {
                current, reason, ..
            } => {
                warn!(state = %current, %reason, "transition rejected");
                Err(OrchestratorError::InvalidTransition(reason))
            }
            TransitionResult::AlreadyTerminal { .. } => Err(OrchestratorError::AlreadyClosed),
        }
    }

    /// Start the session: initialize adapters and transition to Listening.
    pub async fn start(&mut self) -> Result<(), OrchestratorError> {
        // Idle → Connecting
        self.apply_trigger(Trigger::SessionCreate)?;

        self.emit_event(
            event_types::SESSION_CREATED,
            Severity::Info,
            json!({
                "session_id": self.session_id.to_string(),
                "tenant_id": self.tenant_id.to_string(),
                "channel": self.config.channel,
                "direction": self.config.direction,
            }),
        );

        // Initialize adapters
        {
            let mut asr = self.asr.lock().await;
            asr.initialize()
                .await
                .map_err(|e| OrchestratorError::AdapterInitFailed(e.to_string()))?;
        }
        {
            let mut agent = self.agent.lock().await;
            agent
                .initialize()
                .await
                .map_err(|e| OrchestratorError::AdapterInitFailed(e.to_string()))?;
        }
        {
            let mut tts = self.tts.lock().await;
            tts.initialize()
                .await
                .map_err(|e| OrchestratorError::AdapterInitFailed(e.to_string()))?;
        }

        // Connecting → Listening
        self.apply_trigger(Trigger::AdaptersReady)?;

        self.metrics.inc(metric_names::SESSION_CREATED_TOTAL);
        self.metrics.gauge_inc(metric_names::ACTIVE_SESSIONS);

        info!(session_id = %self.session_id, "session started, listening");
        Ok(())
    }

    /// Execute a single turn: user speech → ASR → Agent → TTS.
    /// If `override_transcript` is provided, skip ASR and use it directly.
    /// Returns the agent's response text.
    pub async fn execute_turn(
        &mut self,
        simulated_audio: Option<Vec<u8>>,
    ) -> Result<String, OrchestratorError> {
        self.execute_turn_with_text(simulated_audio, None).await
    }

    /// Execute a turn with optional text override (skips ASR when text is provided).
    pub async fn execute_turn_with_text(
        &mut self,
        simulated_audio: Option<Vec<u8>>,
        override_transcript: Option<String>,
    ) -> Result<String, OrchestratorError> {
        // Listening → UserSpeaking (VAD detected)
        self.apply_trigger(Trigger::VadStarted)?;

        let final_transcript;

        if let Some(text) = override_transcript {
            // Text mode: skip ASR, use provided text directly
            self.apply_trigger(Trigger::VadEnded)?;
            final_transcript = text;
            self.emit_event(
                event_types::TRANSCRIPT_FINAL,
                Severity::Info,
                json!({
                    "speech_id": "text-input",
                    "transcript": &final_transcript,
                    "confidence": 1.0,
                    "language": self.config.language,
                }),
            );
        } else {
            // Audio mode: run ASR pipeline
            let asr = self.asr.clone();
            let (audio_tx, mut result_rx) = {
                let asr_guard = asr.lock().await;
                asr_guard
                    .start_stream(&self.config.language)
                    .await
                    .map_err(|e| OrchestratorError::Internal(e.to_string()))?
            };

            if let Some(audio) = simulated_audio {
                let chunk = prx_voice_adapter::asr::AudioChunk {
                    data: audio,
                    sample_rate: 16000,
                    channels: 1,
                    timestamp_ms: 0,
                };
                let _ = audio_tx.send(chunk).await;
            }
            drop(audio_tx);

            self.apply_trigger(Trigger::VadEnded)?;

            let mut transcript = String::new();
            while let Some(result) = result_rx.recv().await {
                if result.is_final {
                    transcript = result.transcript;
                    if let Some(latency) = result.asr_latency_ms {
                        self.metrics
                            .observe(metric_names::ASR_FINAL_LATENCY_MS, latency as f64);
                    }
                    self.emit_event(
                        event_types::TRANSCRIPT_FINAL,
                        Severity::Info,
                        json!({
                            "speech_id": result.speech_id,
                            "transcript": &transcript,
                            "confidence": result.confidence,
                            "language": result.language,
                        }),
                    );
                    break;
                }
            }
            final_transcript = transcript;
        }

        let is_empty = final_transcript.is_empty();
        self.apply_trigger(Trigger::TranscriptFinal { is_empty })?;

        if is_empty {
            // Empty transcript → back to Listening
            return Ok(String::new());
        }

        // AsrProcessing → Thinking
        // Now get agent response
        let agent = self.agent.clone();
        let ctx = AgentContext {
            session_id: self.session_id.to_string(),
            turn_id: self.current_turn.as_u32(),
            language: self.config.language.clone(),
            system_prompt: None,
            history: self.conversation_history.clone(),
        };

        let mut token_rx = {
            let agent_guard = agent.lock().await;
            agent_guard
                .generate(&final_transcript, &ctx)
                .await
                .map_err(|e| OrchestratorError::Internal(e.to_string()))?
        };

        // Thinking → Speaking (first token)
        let mut agent_response = String::new();
        let mut got_first_token = false;

        while let Some(token) = token_rx.recv().await {
            if !got_first_token {
                self.apply_trigger(Trigger::AgentResponseStart)?;
                got_first_token = true;
            }
            agent_response = token.cumulative_text;
        }

        if !got_first_token {
            // No tokens received — agent timeout/error path
            self.apply_trigger(Trigger::AgentTimeout)?;
            return Err(OrchestratorError::Internal(
                "No agent tokens received".into(),
            ));
        }

        // TTS synthesis
        let tts = self.tts.clone();
        let segment_id = SegmentId::new(self.current_turn, 0);
        let tts_req = TtsSynthesisRequest {
            segment_id: segment_id.to_string(),
            text: agent_response.clone(),
            voice: "default".into(),
            language: self.config.language.clone(),
            speech_rate: None,
            encoding: "pcm".into(),
            sample_rate: 16000,
        };

        let mut chunk_rx = {
            let tts_guard = tts.lock().await;
            tts_guard
                .synthesize(tts_req)
                .await
                .map_err(|e| OrchestratorError::Internal(e.to_string()))?
        };

        // Consume TTS chunks (in Phase 1, we just drain them)
        while let Some(_chunk) = chunk_rx.recv().await {
            // In production: send to playback buffer
        }

        // Speaking → Listening (playback completed)
        self.apply_trigger(Trigger::PlaybackCompleted)?;

        // Emit billing-relevant usage data as event
        self.emit_event(
            event_types::USAGE_RECORDED,
            Severity::Info,
            json!({
                "turn_id": self.current_turn.as_u32(),
                "asr_provider": "mock",
                "agent_provider": "mock",
                "tts_provider": "mock",
            }),
        );

        // Update conversation history
        self.conversation_history.push(ConversationTurn {
            role: "user".into(),
            content: final_transcript,
        });
        self.conversation_history.push(ConversationTurn {
            role: "assistant".into(),
            content: agent_response.clone(),
        });

        // Advance turn
        self.current_turn = self.current_turn.next();

        Ok(agent_response)
    }

    /// Pause the session.
    pub async fn pause(&mut self) -> Result<(), OrchestratorError> {
        self.apply_trigger(Trigger::PauseRequest)?;
        info!(session_id = %self.session_id, "session paused");
        Ok(())
    }

    /// Resume the session from paused state.
    pub async fn resume(&mut self) -> Result<(), OrchestratorError> {
        self.apply_trigger(Trigger::ResumeRequest)?;
        info!(session_id = %self.session_id, "session resumed");
        Ok(())
    }

    /// Close the session.
    pub async fn close(&mut self, reason: &str) -> Result<(), OrchestratorError> {
        self.apply_trigger(Trigger::CloseRequest { force: false })?;

        self.metrics.inc(metric_names::SESSION_CLOSED_TOTAL);
        self.metrics.gauge_dec(metric_names::ACTIVE_SESSIONS);

        self.emit_event(
            event_types::SESSION_CLOSED,
            Severity::Info,
            json!({
                "reason": reason,
                "close_code": 200,
                "initiated_by": "system",
                "total_turns": self.current_turn.as_u32() - 1,
            }),
        );

        info!(session_id = %self.session_id, "session closed");
        Ok(())
    }

    /// Interrupt the current turn (during Speaking state).
    pub async fn interrupt(&mut self) -> Result<(), OrchestratorError> {
        self.apply_trigger(Trigger::InterruptDetected)?;

        self.metrics.inc(metric_names::INTERRUPTS_TOTAL);

        self.emit_event(
            event_types::INTERRUPT_ISSUED,
            Severity::Info,
            json!({
                "interrupted_turn_id": self.current_turn.as_u32(),
                "trigger": "user_barge_in",
            }),
        );

        // Cancel adapters
        {
            let asr = self.asr.lock().await;
            let _ = asr.cancel().await;
        }
        {
            let agent = self.agent.lock().await;
            let _ = agent.cancel().await;
        }
        {
            let tts = self.tts.lock().await;
            let _ = tts.cancel().await;
        }

        // Resolve interrupt → back to Listening
        self.apply_trigger(Trigger::InterruptResolved)?;
        self.current_turn = self.current_turn.next();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prx_voice_adapter::mock_agent::{MockAgent, MockAgentConfig};
    use prx_voice_adapter::mock_asr::{MockAsr, MockAsrConfig};
    use prx_voice_adapter::mock_tts::{MockTts, MockTtsConfig};
    use prx_voice_event::bus::{EventBus, EventBusConfig};
    use prx_voice_observe::metrics::MetricsRegistry;

    fn test_orchestrator() -> SessionOrchestrator {
        let config = SessionConfig::default();
        let asr = Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 10,
            ..Default::default()
        }));
        let agent = Box::new(MockAgent::new(MockAgentConfig {
            first_token_latency_ms: 10,
            ..Default::default()
        }));
        let tts = Box::new(MockTts::new(MockTtsConfig {
            first_chunk_latency_ms: 10,
            ..Default::default()
        }));
        let event_bus = EventBus::new(EventBusConfig::default());
        let metrics = Arc::new(MetricsRegistry::new());

        SessionOrchestrator::new(TenantId::new(), config, asr, agent, tts, event_bus, metrics)
    }

    #[tokio::test]
    async fn full_session_lifecycle() {
        let mut orch = test_orchestrator();

        // Start session
        orch.start().await.unwrap();
        assert_eq!(orch.state(), SessionState::Listening);

        // Execute a turn
        let response = orch.execute_turn(Some(vec![0u8; 1600])).await.unwrap();
        assert!(!response.is_empty());
        assert_eq!(orch.state(), SessionState::Listening);
        assert_eq!(orch.current_turn().as_u32(), 2);

        // Close session
        orch.close("normal_clearing").await.unwrap();
        assert_eq!(orch.state(), SessionState::Closed);
    }

    #[tokio::test]
    async fn multi_turn_session() {
        let mut orch = test_orchestrator();
        orch.start().await.unwrap();

        // Turn 1
        let r1 = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
        assert!(!r1.is_empty());

        // Turn 2
        let r2 = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
        assert!(!r2.is_empty());
        assert_eq!(orch.current_turn().as_u32(), 3);

        orch.close("normal_clearing").await.unwrap();
    }

    #[tokio::test]
    async fn event_bus_receives_events() {
        let event_bus = EventBus::new(EventBusConfig::default());
        let mut sub = event_bus.subscribe();

        let config = SessionConfig::default();
        let asr = Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 10,
            ..Default::default()
        }));
        let agent = Box::new(MockAgent::new(MockAgentConfig {
            first_token_latency_ms: 10,
            ..Default::default()
        }));
        let tts = Box::new(MockTts::new(MockTtsConfig {
            first_chunk_latency_ms: 10,
            ..Default::default()
        }));

        let metrics = Arc::new(MetricsRegistry::new());
        let mut orch =
            SessionOrchestrator::new(TenantId::new(), config, asr, agent, tts, event_bus, metrics);

        orch.start().await.unwrap();

        // Should have received: StateChanged(Idle→Connecting), SessionCreated, StateChanged(Connecting→Listening)
        let e1 = sub.recv().await.unwrap();
        assert_eq!(e1.event_type, event_types::SESSION_STATE_CHANGED);

        let e2 = sub.recv().await.unwrap();
        assert_eq!(e2.event_type, event_types::SESSION_CREATED);

        let e3 = sub.recv().await.unwrap();
        assert_eq!(e3.event_type, event_types::SESSION_STATE_CHANGED);

        // Verify monotonic seq
        assert!(e1.prx_seq < e2.prx_seq);
        assert!(e2.prx_seq < e3.prx_seq);
    }
}
