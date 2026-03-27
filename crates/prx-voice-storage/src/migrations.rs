//! PostgreSQL migration SQL statements.
//! Applied in order by the migration runner.

pub const MIGRATIONS: &[(&str, &str)] = &[
    ("001_create_sessions", CREATE_SESSIONS),
    ("002_create_turns", CREATE_TURNS),
    ("003_create_events", CREATE_EVENTS),
    ("004_create_audit", CREATE_AUDIT),
    ("005_create_billing", CREATE_BILLING),
];

const CREATE_SESSIONS: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    session_id    UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL,
    state         VARCHAR(32) NOT NULL DEFAULT 'Idle',
    channel       VARCHAR(32) NOT NULL,
    direction     VARCHAR(16) NOT NULL DEFAULT 'inbound',
    language      VARCHAR(16) NOT NULL DEFAULT 'en-US',
    total_turns   INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at     TIMESTAMPTZ,
    close_reason  VARCHAR(128),
    metadata      JSONB
);

CREATE INDEX IF NOT EXISTS idx_sessions_tenant ON sessions(tenant_id);
CREATE INDEX IF NOT EXISTS idx_sessions_state ON sessions(state);
CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at DESC);
"#;

const CREATE_TURNS: &str = r#"
CREATE TABLE IF NOT EXISTS turns (
    turn_id           UUID PRIMARY KEY,
    session_id        UUID NOT NULL REFERENCES sessions(session_id),
    tenant_id         UUID NOT NULL,
    sequence_no       INTEGER NOT NULL,
    user_transcript   TEXT,
    agent_response    TEXT,
    asr_latency_ms    BIGINT,
    agent_latency_ms  BIGINT,
    tts_latency_ms    BIGINT,
    interrupted       BOOLEAN NOT NULL DEFAULT FALSE,
    started_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id);
CREATE INDEX IF NOT EXISTS idx_turns_tenant ON turns(tenant_id);
"#;

const CREATE_EVENTS: &str = r#"
CREATE TABLE IF NOT EXISTS session_events (
    event_id     UUID PRIMARY KEY,
    session_id   UUID NOT NULL,
    tenant_id    UUID NOT NULL,
    turn_id      INTEGER,
    seq          BIGINT NOT NULL,
    event_type   VARCHAR(128) NOT NULL,
    severity     VARCHAR(16) NOT NULL DEFAULT 'info',
    payload      JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_session_seq ON session_events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_tenant ON session_events(tenant_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON session_events(event_type);
"#;

const CREATE_AUDIT: &str = r#"
CREATE TABLE IF NOT EXISTS audit_records (
    audit_id       UUID PRIMARY KEY,
    tenant_id      UUID,
    principal_id   VARCHAR(256) NOT NULL,
    principal_type VARCHAR(32) NOT NULL,
    action         VARCHAR(64) NOT NULL,
    target_type    VARCHAR(64) NOT NULL,
    target_id      VARCHAR(256) NOT NULL,
    result         VARCHAR(16) NOT NULL,
    reason         TEXT,
    correlation_id VARCHAR(256),
    details        JSONB,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_tenant ON audit_records(tenant_id);
CREATE INDEX IF NOT EXISTS idx_audit_principal ON audit_records(principal_id);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_records(action);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_records(created_at DESC);
"#;

const CREATE_BILLING: &str = r#"
CREATE TABLE IF NOT EXISTS billing_entries (
    entry_id         UUID PRIMARY KEY,
    idempotency_key  VARCHAR(256) NOT NULL UNIQUE,
    tenant_id        UUID NOT NULL,
    session_id       UUID,
    meter_type       VARCHAR(64) NOT NULL,
    quantity         DOUBLE PRECISION NOT NULL,
    unit             VARCHAR(32) NOT NULL,
    provider         VARCHAR(64),
    entry_type       VARCHAR(16) NOT NULL DEFAULT 'usage',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_billing_tenant ON billing_entries(tenant_id);
CREATE INDEX IF NOT EXISTS idx_billing_meter ON billing_entries(meter_type);
CREATE INDEX IF NOT EXISTS idx_billing_idempotency ON billing_entries(idempotency_key);
"#;
