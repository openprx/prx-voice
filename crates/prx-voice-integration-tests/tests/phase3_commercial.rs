//! Phase 3 integration tests: commercial capabilities.

use prx_voice_adapter::fallback::FallbackAsr;
use prx_voice_adapter::mock_agent::{MockAgent, MockAgentConfig};
use prx_voice_adapter::mock_asr::{MockAsr, MockAsrConfig};
use prx_voice_adapter::mock_tts::{MockTts, MockTtsConfig};
use prx_voice_audit::record::{AuditAction, AuditRecord, AuditResult, PrincipalType};
use prx_voice_audit::store::AuditStore;
use prx_voice_billing::ledger::{BillingLedger, usage_entry};
use prx_voice_billing::meter::MeterType;
use prx_voice_event::bus::{EventBus, EventBusConfig};
use prx_voice_observe::metrics::MetricsRegistry;
use prx_voice_policy::quota::{QuotaCheckResult, QuotaLimits, QuotaTracker};
use prx_voice_policy::tenant::{TenantPolicy, TenantPolicyStore, TenantTier};
use prx_voice_session::config::SessionConfig;
use prx_voice_session::manager::{SessionManager, SessionManagerError, TenantLimits};
use prx_voice_types::ids::TenantId;
use std::sync::Arc;

fn mock_adapters() -> (
    Box<dyn prx_voice_adapter::asr::AsrAdapter>,
    Box<dyn prx_voice_adapter::agent::AgentAdapter>,
    Box<dyn prx_voice_adapter::tts::TtsAdapter>,
) {
    (
        Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 5,
            ..Default::default()
        })),
        Box::new(MockAgent::new(MockAgentConfig {
            first_token_latency_ms: 5,
            ..Default::default()
        })),
        Box::new(MockTts::new(MockTtsConfig {
            first_chunk_latency_ms: 5,
            ..Default::default()
        })),
    )
}

#[tokio::test]
async fn multi_tenant_session_isolation() {
    let bus = EventBus::new(EventBusConfig::default());
    let mgr = SessionManager::new(
        bus,
        TenantLimits {
            max_concurrent_sessions: 2,
        },
        Arc::new(MetricsRegistry::new()),
    );

    let t1 = TenantId::new();
    let t2 = TenantId::new();

    // Tenant 1: create 2 sessions (at limit)
    for _ in 0..2 {
        let (asr, agent, tts) = mock_adapters();
        mgr.create_session(t1, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();
    }

    // Tenant 1: third should fail
    let (asr, agent, tts) = mock_adapters();
    assert!(matches!(
        mgr.create_session(t1, SessionConfig::default(), asr, agent, tts)
            .await,
        Err(SessionManagerError::ConcurrentLimitExceeded { .. })
    ));

    // Tenant 2: should still work (independent)
    let (asr, agent, tts) = mock_adapters();
    mgr.create_session(t2, SessionConfig::default(), asr, agent, tts)
        .await
        .unwrap();

    assert_eq!(mgr.tenant_session_count(t1), 2);
    assert_eq!(mgr.tenant_session_count(t2), 1);
    assert_eq!(mgr.total_session_count(), 3);
}

#[tokio::test]
async fn fallback_adapter_in_session() {
    let bus = EventBus::new(EventBusConfig::default());

    // Primary ASR fails, backup succeeds
    let primary = Box::new(MockAsr::new(MockAsrConfig {
        inject_error: true,
        ..Default::default()
    }));
    let backup = Box::new(MockAsr::new(MockAsrConfig {
        latency_ms: 5,
        transcript: "fallback transcript".into(),
        ..Default::default()
    }));
    let fallback_asr = Box::new(FallbackAsr::new(vec![primary, backup]));

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
        fallback_asr,
        agent,
        tts,
        bus,
        Arc::new(MetricsRegistry::new()),
    );
    orch.start().await.unwrap();

    let response = orch.execute_turn(Some(vec![0u8; 100])).await.unwrap();
    assert!(!response.is_empty());
    orch.close("test").await.unwrap();
}

#[test]
fn audit_trail_for_session_lifecycle() {
    let store = AuditStore::new();
    let tid = TenantId::new();

    // Simulate session lifecycle audit events
    store.append(
        AuditRecord::new(
            "system",
            PrincipalType::System,
            AuditAction::SessionCreated,
            "session",
            "sess-1",
            AuditResult::Success,
        )
        .with_tenant(tid),
    );
    store.append(
        AuditRecord::new(
            "system",
            PrincipalType::System,
            AuditAction::SessionClosed,
            "session",
            "sess-1",
            AuditResult::Success,
        )
        .with_tenant(tid)
        .with_reason("normal_clearing"),
    );

    let records = store.query(&prx_voice_audit::store::AuditQuery {
        tenant_id: Some(tid),
        ..Default::default()
    });
    assert_eq!(records.len(), 2);
}

#[test]
fn billing_ledger_tracks_session_usage() {
    let ledger = BillingLedger::new();
    let tid = TenantId::new();

    // Record ASR usage
    ledger
        .record(usage_entry(
            tid,
            None,
            MeterType::AsrAudioSeconds,
            45.0,
            Some("deepgram".into()),
        ))
        .unwrap();
    // Record Agent tokens
    ledger
        .record(usage_entry(
            tid,
            None,
            MeterType::AgentInputTokens,
            450.0,
            Some("openai".into()),
        ))
        .unwrap();
    ledger
        .record(usage_entry(
            tid,
            None,
            MeterType::AgentOutputTokens,
            280.0,
            Some("openai".into()),
        ))
        .unwrap();
    // Record TTS
    ledger
        .record(usage_entry(
            tid,
            None,
            MeterType::TtsCharacters,
            1200.0,
            Some("azure".into()),
        ))
        .unwrap();

    let summary = ledger.summarize(tid);
    assert_eq!(summary[&MeterType::AsrAudioSeconds], 45.0);
    assert_eq!(summary[&MeterType::AgentInputTokens], 450.0);
    assert_eq!(summary[&MeterType::AgentOutputTokens], 280.0);
    assert_eq!(summary[&MeterType::TtsCharacters], 1200.0);
}

#[test]
fn tenant_policy_and_quota_enforcement() {
    let policy_store = TenantPolicyStore::new();
    let quota_tracker = QuotaTracker::new();

    let tid = TenantId::new();
    let policy = TenantPolicy::for_tier(tid, TenantTier::Trial);
    policy_store.set(policy.clone());

    // Set quota limits based on policy
    quota_tracker.set_limits(
        tid,
        QuotaLimits {
            max_concurrent_sessions: policy.max_concurrent_sessions,
            ..Default::default()
        },
    );

    // Trial tier: max 2 concurrent
    quota_tracker.record_session_start(tid);
    quota_tracker.record_session_start(tid);
    assert!(matches!(
        quota_tracker.check_session_create(tid),
        QuotaCheckResult::Exceeded { .. }
    ));

    // End a session, should allow another
    quota_tracker.record_session_end(tid, 60.0);
    assert_eq!(
        quota_tracker.check_session_create(tid),
        QuotaCheckResult::Allowed
    );
}
