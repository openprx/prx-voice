//! REST API handlers.

use crate::ratelimit::{RateLimitConfig, RateLimiter};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use prx_voice_adapter::factory;
use prx_voice_audit::record::{
    AuditAction, AuditRecord, AuditResult as AuditOutcome, PrincipalType,
};
use prx_voice_audit::store::AuditQuery;
use prx_voice_event::bus::EventBus;
use prx_voice_session::config::SessionConfig;
use prx_voice_session::orchestrator::SessionOrchestrator;
use prx_voice_types::ids::{SessionId, TenantId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tracing::error;
use uuid::Uuid;

static RATE_LIMITER: LazyLock<RateLimiter> = LazyLock::new(|| {
    RateLimiter::new(RateLimitConfig {
        max_requests: 300,
        window: std::time::Duration::from_secs(60),
    })
});

/// Unified API response envelope.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub request_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

fn success_response<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        request_id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        data: Some(data),
        error: None,
    })
}

fn error_response(status: StatusCode, code: &str, message: &str) -> impl IntoResponse {
    (
        status,
        Json(ApiResponse::<()> {
            request_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                retryable: false,
            }),
        }),
    )
}

/// Axum middleware that enforces per-tenant rate limiting and appends
/// `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `Retry-After` headers.
async fn rate_limit_middleware(request: axum::http::Request<Body>, next: Next) -> Response<Body> {
    // Skip rate limiting for health and WebSocket endpoints
    let path = request.uri().path();
    if path.starts_with("/api/v1/health") || path.ends_with("/stream") || path.ends_with("/events")
    {
        return next.run(request).await;
    }

    // Use tenant header or fall back to "anonymous"
    let key = request
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    let info = RATE_LIMITER.check(&key);

    if !info.allowed {
        let body = serde_json::json!({
            "error": {
                "code": "RATE_LIMIT_EXCEEDED",
                "message": "Too many requests",
                "retryable": true
            }
        })
        .to_string();

        let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
        let headers = response.headers_mut();
        if let Ok(v) = HeaderValue::from_str(&info.limit.to_string()) {
            headers.insert("X-RateLimit-Limit", v);
        }
        if let Ok(v) = HeaderValue::from_str("0") {
            headers.insert("X-RateLimit-Remaining", v);
        }
        if let Ok(v) = HeaderValue::from_str(&info.reset_after_sec.to_string()) {
            headers.insert("Retry-After", v);
        }
        return response;
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&info.limit.to_string()) {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&info.remaining.to_string()) {
        headers.insert("X-RateLimit-Remaining", v);
    }
    response
}

/// Build the API router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/sessions", get(list_sessions).post(create_session))
        .route("/api/v1/sessions/{session_id}", get(get_session))
        .route(
            "/api/v1/sessions/{session_id}/turns",
            get(list_turns).post(execute_turn),
        )
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/sessions/{session_id}/close", post(close_session))
        .route(
            "/api/v1/sessions/{session_id}/interrupt",
            post(interrupt_session),
        )
        .route("/api/v1/sessions/{session_id}/pause", post(pause_session))
        .route("/api/v1/sessions/{session_id}/resume", post(resume_session))
        .route("/api/v1/sessions/{session_id}/events", get(events_ws))
        .route(
            "/api/v1/sessions/{session_id}/stream",
            get(session_stream_ws),
        )
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/billing/summary", get(billing_summary))
        .route("/api/v1/health", get(health))
        .route("/api/v1/health/live", get(health_live))
        .route("/api/v1/health/ready", get(health_ready))
        .layer(middleware::from_fn(rate_limit_middleware))
        .with_state(state)
}

// --- Request / Response types ---

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    /// ASR providers (first = primary, rest = fallback). Default: ["mock"]
    #[serde(default)]
    pub asr_providers: Option<Vec<String>>,
    /// Agent providers. Default: ["mock"]
    #[serde(default)]
    pub agent_providers: Option<Vec<String>>,
    /// TTS providers. Default: ["mock"]
    #[serde(default)]
    pub tts_providers: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub state: String,
    pub channel: String,
    pub language: String,
    pub current_turn_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct CloseSessionRequest {
    #[serde(default = "default_close_reason")]
    pub reason: String,
}

fn default_close_reason() -> String {
    "normal_clearing".into()
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub items: Vec<SessionInfo>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub has_more: bool,
    pub total_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteTurnRequest {
    /// Optional text to simulate user speech (mock ASR will use its default if not provided).
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TurnResult {
    pub session_id: String,
    pub turn_id: u32,
    pub state: String,
    pub user_transcript: String,
    pub agent_response: String,
}

// --- Handlers ---

async fn create_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    // Check idempotency key — if a session was already created with this key, return 409 Conflict.
    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    if let Some(ref key) = idempotency_key {
        let store = state.idempotency.read();
        if let Some(existing_session_id) = store.get(key) {
            return error_response(
                StatusCode::CONFLICT,
                "IDEMPOTENT_REQUEST_DUPLICATE",
                &format!("Session already created with id {existing_session_id}"),
            )
            .into_response();
        }
    }

    let mut config = SessionConfig::default();
    if let Some(channel) = req.channel {
        config.channel = channel;
    }
    if let Some(language) = req.language {
        config.language = language;
    }

    let default_providers = vec!["mock".to_string()];
    let asr_providers = req
        .asr_providers
        .unwrap_or_else(|| default_providers.clone());
    let agent_providers = req
        .agent_providers
        .unwrap_or_else(|| default_providers.clone());
    let tts_providers = req.tts_providers.unwrap_or(default_providers);

    let asr = factory::create_asr_with_fallback(&asr_providers);
    let agent = factory::create_agent_with_fallback(&agent_providers);
    let tts = factory::create_tts_with_fallback(&tts_providers);

    let mut orch = SessionOrchestrator::new(
        TenantId::new(),
        config.clone(),
        asr,
        agent,
        tts,
        state.event_bus.clone(),
        Arc::clone(&state.metrics),
    );

    let session_id = orch.session_id();

    if let Err(e) = orch.start().await {
        error!(%session_id, error = %e, "failed to start session");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SESSION_CREATE_FAILED",
            &e.to_string(),
        )
        .into_response();
    }

    let info = SessionInfo {
        session_id: session_id.to_string(),
        state: orch.state().to_string(),
        channel: config.channel,
        language: config.language,
        current_turn_id: orch.current_turn().as_u32(),
    };

    state
        .sessions
        .write()
        .insert(session_id, Arc::new(Mutex::new(orch)));

    // Store idempotency key mapping so duplicate requests are detected.
    if let Some(key) = idempotency_key {
        state
            .idempotency
            .write()
            .insert(key, info.session_id.clone());
    }

    state.audit.append(AuditRecord::new(
        "api",
        PrincipalType::System,
        AuditAction::SessionCreated,
        "session",
        &info.session_id,
        AuditOutcome::Success,
    ));

    (StatusCode::CREATED, success_response(info)).into_response()
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let orch = orch_arc.lock().await;
    let info = SessionInfo {
        session_id: orch.session_id().to_string(),
        state: orch.state().to_string(),
        channel: "web_console".into(),
        language: "en-US".into(),
        current_turn_id: orch.current_turn().as_u32(),
    };

    success_response(info).into_response()
}

async fn close_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
    Json(req): Json<CloseSessionRequest>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let mut orch = orch_arc.lock().await;
    if let Err(e) = orch.close(&req.reason).await {
        return error_response(StatusCode::CONFLICT, "SESSION_CLOSE_FAILED", &e.to_string())
            .into_response();
    }

    let info = SessionInfo {
        session_id: orch.session_id().to_string(),
        state: orch.state().to_string(),
        channel: "web_console".into(),
        language: "en-US".into(),
        current_turn_id: orch.current_turn().as_u32(),
    };

    state.audit.append(
        AuditRecord::new(
            "api",
            PrincipalType::System,
            AuditAction::SessionClosed,
            "session",
            &info.session_id,
            AuditOutcome::Success,
        )
        .with_reason(&req.reason),
    );

    success_response(info).into_response()
}

async fn interrupt_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let mut orch = orch_arc.lock().await;
    if let Err(e) = orch.interrupt().await {
        return error_response(StatusCode::CONFLICT, "INTERRUPT_FAILED", &e.to_string())
            .into_response();
    }

    let info = SessionInfo {
        session_id: orch.session_id().to_string(),
        state: orch.state().to_string(),
        channel: "web_console".into(),
        language: "en-US".into(),
        current_turn_id: orch.current_turn().as_u32(),
    };

    success_response(info).into_response()
}

async fn pause_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let mut orch = orch_arc.lock().await;
    if let Err(e) = orch.pause().await {
        return error_response(StatusCode::CONFLICT, "PAUSE_FAILED", &e.to_string())
            .into_response();
    }

    let info = SessionInfo {
        session_id: orch.session_id().to_string(),
        state: orch.state().to_string(),
        channel: "web_console".into(),
        language: "en-US".into(),
        current_turn_id: orch.current_turn().as_u32(),
    };

    state.audit.append(
        AuditRecord::new(
            "api",
            PrincipalType::System,
            AuditAction::SessionPaused,
            "session",
            &info.session_id,
            AuditOutcome::Success,
        )
        .with_reason("paused"),
    );

    success_response(info).into_response()
}

async fn resume_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let mut orch = orch_arc.lock().await;
    if let Err(e) = orch.resume().await {
        return error_response(StatusCode::CONFLICT, "RESUME_FAILED", &e.to_string())
            .into_response();
    }

    let info = SessionInfo {
        session_id: orch.session_id().to_string(),
        state: orch.state().to_string(),
        channel: "web_console".into(),
        language: "en-US".into(),
        current_turn_id: orch.current_turn().as_u32(),
    };

    state.audit.append(
        AuditRecord::new(
            "api",
            PrincipalType::System,
            AuditAction::SessionResumed,
            "session",
            &info.session_id,
            AuditOutcome::Success,
        )
        .with_reason("resumed"),
    );

    success_response(info).into_response()
}

async fn events_ws(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id format",
            )
            .into_response();
        }
    };

    // Verify session exists
    {
        let sessions = state.sessions.read();
        if !sessions.contains_key(&session_id) {
            return error_response(
                StatusCode::NOT_FOUND,
                "SESSION_NOT_FOUND",
                "Session does not exist",
            )
            .into_response();
        }
    }

    let event_bus = state.event_bus.clone();
    ws.on_upgrade(move |socket| handle_event_stream(socket, event_bus, session_id))
        .into_response()
}

async fn handle_event_stream(mut socket: WebSocket, event_bus: EventBus, session_id: SessionId) {
    let mut subscriber = event_bus.subscribe();
    let mut heartbeat = interval(Duration::from_secs(15));
    let mut last_seq: u64 = 0;

    loop {
        tokio::select! {
            event = subscriber.recv() => {
                match event {
                    Some(evt) if evt.prx_session_id == session_id => {
                        last_seq = evt.prx_seq;
                        let msg = serde_json::json!({
                            "type": "event",
                            "seq": evt.prx_seq,
                            "event": evt,
                        });
                        if let Ok(text) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
            _ = heartbeat.tick() => {
                let hb = serde_json::json!({
                    "type": "heartbeat",
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "last_seq": last_seq,
                });
                if let Ok(text) = serde_json::to_string(&hb) {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    // Socket is dropped here, which closes the connection.
}

/// Full-duplex WebSocket for real-time voice:
/// - Client sends: Binary(PCM audio frames) + Text(JSON commands like {"type":"end_turn"})
/// - Server sends: Binary(PCM audio from TTS) + Text(JSON events/transcript/response)
async fn session_stream_ws(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let event_bus = state.event_bus.clone();

    ws.on_upgrade(move |socket| handle_realtime_stream(socket, orch_arc, event_bus, session_id))
        .into_response()
}

async fn handle_realtime_stream(
    socket: WebSocket,
    orch: Arc<Mutex<SessionOrchestrator>>,
    event_bus: EventBus,
    session_id: SessionId,
) {
    let (ws_sink, mut ws_stream) = socket.split();
    let mut event_sub = event_bus.subscribe();

    // Shared sink so both the event-forwarding task and the main loop can send
    let sink = Arc::new(tokio::sync::Mutex::new(ws_sink));
    let sink_events = sink.clone();

    // Task 1: Forward events to client as JSON text messages
    let event_task = tokio::spawn(async move {
        let mut heartbeat = interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                event = event_sub.recv() => {
                    match event {
                        Some(evt) if evt.prx_session_id == session_id => {
                            let msg = serde_json::json!({
                                "type": "event",
                                "seq": evt.prx_seq,
                                "event_type": evt.event_type,
                                "data": evt.data,
                                "timestamp": evt.time.to_rfc3339(),
                            });
                            if let Ok(text) = serde_json::to_string(&msg) {
                                let mut s = sink_events.lock().await;
                                if s.send(Message::Text(text.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
                _ = heartbeat.tick() => {
                    let hb = serde_json::json!({"type": "heartbeat"});
                    if let Ok(text) = serde_json::to_string(&hb) {
                        let mut s = sink_events.lock().await;
                        if s.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Task 2: Receive from client (audio binary + text commands)
    let sink_main = sink.clone();
    let orch_clone = orch.clone();
    let mut audio_buffer: Vec<u8> = Vec::new();
    let http_client = reqwest::Client::new();
    // Translate mode: user speaks Chinese, AI replies in English
    let translate_mode = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Clone voice mode
    let clone_mode = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let clone_speaker = Arc::new(parking_lot::Mutex::new(String::new()));

    // Pre-init ASR engine once for this stream
    use prx_voice_adapter::local::engine::{AsrAudioInput, AsrEngineConfig, HttpAsrEngine, LocalAsrEngine};
    let mut asr_engine = HttpAsrEngine::new("http://localhost:8765");
    {
        let config = AsrEngineConfig {
            engine: "http".into(),
            model_path: None,
            language: "zh-CN".into(),
            sample_rate: 16000,
            streaming: true,
        };
        if let Err(e) = asr_engine.init(&config).await {
            tracing::warn!(error = %e, "HTTP ASR engine init failed — start python3 models/asr_server.py");
        }
    }

    while let Some(msg_result) = ws_stream.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                // JSON command from client
                if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                    let cmd_type = cmd.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match cmd_type {
                        "set_translate" => {
                            let enabled = cmd.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                            translate_mode.store(enabled, std::sync::atomic::Ordering::Relaxed);
                            ws_send_json(&sink_main, serde_json::json!({
                                "type": "status",
                                "status": if enabled { "translate_on" } else { "translate_off" },
                            })).await;
                        }
                        "set_clone" => {
                            let enabled = cmd.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                            clone_mode.store(enabled, std::sync::atomic::Ordering::Relaxed);
                            if let Some(spk) = cmd.get("speaker").and_then(|v| v.as_str()) {
                                *clone_speaker.lock() = spk.to_string();
                            }
                            ws_send_json(&sink_main, serde_json::json!({
                                "type": "status",
                                "status": if enabled { "clone_on" } else { "clone_off" },
                            })).await;
                        }
                        "text" => {
                            let user_text = cmd
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if user_text.is_empty() {
                                continue;
                            }
                            let is_translate = translate_mode.load(std::sync::atomic::Ordering::Relaxed);
                            let is_clone = clone_mode.load(std::sync::atomic::Ordering::Relaxed);
                            let spk = clone_speaker.lock().clone();
                            streaming_turn(
                                &sink_main, &orch_clone, &http_client, user_text,
                                is_translate, if is_clone { Some(spk) } else { None },
                            ).await;
                        }
                        "audio_start" => {
                            audio_buffer.clear();
                            let msg = serde_json::json!({"type": "status", "status": "recording"});
                            if let Ok(text) = serde_json::to_string(&msg) {
                                let mut s = sink_main.lock().await;
                                let _ = s.send(Message::Text(text.into())).await;
                            }
                        }
                        "audio_end" => {
                            // Process accumulated audio
                            let audio = std::mem::take(&mut audio_buffer);
                            if audio.is_empty() {
                                let msg = serde_json::json!({"type": "error", "message": "No audio received"});
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    let mut s = sink_main.lock().await;
                                    let _ = s.send(Message::Text(text.into())).await;
                                }
                                continue;
                            }
                            {
                                let msg = serde_json::json!({"type": "status", "status": "thinking"});
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    let mut s = sink_main.lock().await;
                                    let _ = s.send(Message::Text(text.into())).await;
                                }
                            }
                            // ASR: recognize audio via HTTP ASR server
                            let pcm: Vec<i16> = audio
                                .chunks_exact(2)
                                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                .collect();
                            let input = AsrAudioInput {
                                pcm_data: pcm,
                                sample_rate: 16000,
                            };
                            let _ = asr_engine.process_audio(&input);
                            let asr_text = asr_engine
                                .finalize_async()
                                .await
                                .ok()
                                .flatten()
                                .map(|r| r.text)
                                .unwrap_or_default();
                            asr_engine.reset();

                            if asr_text.trim().is_empty() || asr_text == "(未识别到语音)" {
                                let msg = serde_json::json!({"type": "no_speech"});
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    let mut s = sink_main.lock().await;
                                    let _ = s.send(Message::Text(text.into())).await;
                                }
                                let msg = serde_json::json!({"type": "status", "status": "listening"});
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    let mut s = sink_main.lock().await;
                                    let _ = s.send(Message::Text(text.into())).await;
                                }
                                continue;
                            }
                            // Send ASR transcript to client
                            {
                                let msg = serde_json::json!({"type": "transcript", "text": &asr_text});
                                if let Ok(text) = serde_json::to_string(&msg) {
                                    let mut s = sink_main.lock().await;
                                    let _ = s.send(Message::Text(text.into())).await;
                                }
                            }
                            // Streaming pipeline: Agent → sentence TTS → audio
                            let is_translate = translate_mode.load(std::sync::atomic::Ordering::Relaxed);
                            let is_clone = clone_mode.load(std::sync::atomic::Ordering::Relaxed);
                            let spk = clone_speaker.lock().clone();
                            streaming_turn(
                                &sink_main, &orch_clone, &http_client, asr_text,
                                is_translate, if is_clone { Some(spk) } else { None },
                            ).await;
                        }
                        "interrupt" => {
                            let mut o = orch_clone.lock().await;
                            let _ = o.interrupt().await;
                            let msg = serde_json::json!({"type": "status", "status": "interrupted"});
                            if let Ok(text) = serde_json::to_string(&msg) {
                                let mut s = sink_main.lock().await;
                                let _ = s.send(Message::Text(text.into())).await;
                            }
                        }
                        "close" => {
                            let mut o = orch_clone.lock().await;
                            let _ = o.close("user_requested").await;
                            let msg = serde_json::json!({"type": "status", "status": "closed"});
                            if let Ok(text) = serde_json::to_string(&msg) {
                                let mut s = sink_main.lock().await;
                                let _ = s.send(Message::Text(text.into())).await;
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Binary(audio_data)) => {
                // Accumulate audio PCM from microphone
                if audio_buffer.len() < 10 * 1024 * 1024 {
                    audio_buffer.extend_from_slice(&audio_data);
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    event_task.abort();
    let mut s = sink.lock().await;
    let _ = s.close().await;
}

/// Send a WS text message helper.
async fn ws_send_json(
    sink: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    value: serde_json::Value,
) {
    if let Ok(text) = serde_json::to_string(&value) {
        let mut s = sink.lock().await;
        let _ = s.send(Message::Text(text.into())).await;
    }
}

/// True streaming turn: orchestrator state → Agent stream → sentence-level TTS → audio stream.
/// When `translate` is true, user speaks Chinese and AI replies in English.
/// When `clone_speaker` is Some, uses clone TTS server with that speaker ID.
async fn streaming_turn(
    sink: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    orch: &Arc<tokio::sync::Mutex<SessionOrchestrator>>,
    http: &reqwest::Client,
    user_text: String,
    translate: bool,
    clone_speaker: Option<String>,
) {
    // Phase 1: Update orchestrator state (Listening → … → Thinking)
    {
        let mut o = orch.lock().await;
        if let Err(e) = o.execute_turn_with_text(None, Some(user_text.clone())).await {
            ws_send_json(sink, serde_json::json!({"type": "error", "message": e.to_string()})).await;
            return;
        }
    }

    ws_send_json(sink, serde_json::json!({"type": "status", "status": "thinking"})).await;

    // Phase 2: Call Ollama directly for streaming tokens
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:1.5b".into());

    let (system_prompt, user_prompt) = if translate {
        (
            "你是一个翻译助手。用户用中文跟你说话，你必须且只能用英文回答。\
             禁止使用任何中文。只用English回复。回答简洁自然，1-3句。",
            format!("请把下面这句话翻译成英文并自然地回应：「{user_text}」\n（只用英文回答）"),
        )
    } else {
        (
            "你是一个语音助手。请用中文回答，回答要简洁自然，像正常对话一样。每次回答控制在2-3句话以内。",
            user_text.clone(),
        )
    };

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": &user_prompt},
        ],
        "stream": true,
        "options": {"temperature": 0.7, "num_predict": 256},
    });

    let response = match http.post(format!("{ollama_url}/api/chat")).json(&body).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let status = r.status();
            ws_send_json(sink, serde_json::json!({"type": "error", "message": format!("Ollama {status}")})).await;
            return;
        }
        Err(e) => {
            ws_send_json(sink, serde_json::json!({"type": "error", "message": format!("Ollama unreachable: {e}")})).await;
            return;
        }
    };

    // Signal TTS start
    ws_send_json(sink, serde_json::json!({"type": "tts_start", "sample_rate": 16000})).await;

    // Phase 3: Stream tokens, accumulate sentences, TTS each sentence
    let mut stream = response.bytes_stream();
    let mut json_buf = String::new();
    let mut full_response = String::new();
    let mut sentence_buf = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(_) => break,
        };
        json_buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = json_buf.find('\n') {
            let line = json_buf[..line_end].trim().to_string();
            json_buf = json_buf[line_end + 1..].to_string();
            if line.is_empty() {
                continue;
            }

            #[derive(serde::Deserialize)]
            struct OllamaChunk {
                message: Option<OllamaMsg>,
                done: Option<bool>,
            }
            #[derive(serde::Deserialize)]
            struct OllamaMsg {
                content: Option<String>,
            }

            if let Ok(parsed) = serde_json::from_str::<OllamaChunk>(&line) {
                let is_done = parsed.done.unwrap_or(false);

                if let Some(msg) = parsed.message {
                    if let Some(content) = msg.content {
                        if !content.is_empty() {
                            full_response.push_str(&content);
                            sentence_buf.push_str(&content);

                            // Send token to client for real-time text display
                            ws_send_json(sink, serde_json::json!({
                                "type": "token",
                                "token": &content,
                                "cumulative": &full_response,
                            })).await;

                            // Check for sentence boundary to trigger TTS
                            if has_sentence_boundary(&sentence_buf) {
                                let sentence = std::mem::take(&mut sentence_buf);
                                tts_and_stream(sink, http, &sentence, translate, &clone_speaker).await;
                            }
                        }
                    }
                }

                if is_done {
                    break;
                }
            }
        }
    }

    // TTS any remaining text
    if !sentence_buf.is_empty() {
        tts_and_stream(sink, http, &sentence_buf, translate, &clone_speaker).await;
    }

    // Signal response complete
    ws_send_json(sink, serde_json::json!({
        "type": "response",
        "user_text": &user_text,
        "agent_text": &full_response,
    })).await;

    // Signal TTS end
    ws_send_json(sink, serde_json::json!({"type": "tts_end"})).await;
}

/// Check if buffer contains a sentence boundary (Chinese/English punctuation).
fn has_sentence_boundary(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let last_char = trimmed.chars().last().unwrap_or(' ');
    matches!(last_char, '。' | '！' | '？' | '；' | '\n' | '.' | '!' | '?' | ';')
}

/// Call HTTP TTS server and stream PCM16 audio back to WebSocket client.
/// Routes to: clone server (:8768) if speaker set, EN TTS (:8767) if translate, else ZH TTS (:8766).
async fn tts_and_stream(
    sink: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    http: &reqwest::Client,
    text: &str,
    translate: bool,
    clone_speaker: &Option<String>,
) {
    // Route to appropriate TTS server
    let use_clone = clone_speaker.as_ref().is_some_and(|s| !s.is_empty());

    let (tts_url, body) = if use_clone {
        let url = std::env::var("TTS_CLONE_URL").unwrap_or_else(|_| "http://localhost:8768".into());
        let lang = if translate { "en" } else { "zh" };
        (url, serde_json::json!({"text": text, "speed": 1.0, "speaker": clone_speaker, "lang": lang}))
    } else if translate {
        let url = std::env::var("TTS_EN_URL").unwrap_or_else(|_| "http://localhost:8767".into());
        (url, serde_json::json!({"text": text, "speed": 1.0}))
    } else {
        let url = std::env::var("TTS_URL").unwrap_or_else(|_| "http://localhost:8766".into());
        (url, serde_json::json!({"text": text, "speed": 1.0}))
    };

    let resp = match http
        .post(format!("{tts_url}/tts"))
        .json(&body)
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!(status = %r.status(), "TTS server error");
            return;
        }
        Err(e) => {
            tracing::warn!(error = %e, "TTS server unreachable");
            return;
        }
    };

    // Send audio as binary WebSocket frames in ~200ms chunks
    let audio_bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => return,
    };

    let chunk_bytes = 16000 * 2 / 5; // 200ms at 16kHz, 16-bit = 6400 bytes
    for chunk in audio_bytes.chunks(chunk_bytes) {
        let mut s = sink.lock().await;
        if s.send(Message::Binary(chunk.to_vec().into())).await.is_err() {
            break;
        }
    }
}

async fn list_sessions(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<ListSessionsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).min(100);

    let (arcs, total) = {
        let sessions = state.sessions.read();
        let total = sessions.len();
        let arcs: Vec<_> = sessions.values().take(limit).cloned().collect();
        (arcs, total)
    };

    let mut items = Vec::new();
    for orch_arc in arcs {
        let orch = orch_arc.lock().await;
        items.push(SessionInfo {
            session_id: orch.session_id().to_string(),
            state: orch.state().to_string(),
            channel: "web_console".into(),
            language: "en-US".into(),
            current_turn_id: orch.current_turn().as_u32(),
        });
    }

    success_response(SessionListResponse {
        items,
        pagination: PaginationInfo {
            has_more: total > limit,
            total_count: total,
            cursor: None,
        },
    })
    .into_response()
}

async fn list_turns(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        sessions.get(&session_id).cloned()
    };

    let Some(orch_arc) = orch_arc else {
        return error_response(
            StatusCode::NOT_FOUND,
            "SESSION_NOT_FOUND",
            "Session does not exist",
        )
        .into_response();
    };

    let orch = orch_arc.lock().await;
    let current_turn = orch.current_turn().as_u32();

    // Return basic turn info (full turn history not tracked in Phase 3)
    let turns: Vec<serde_json::Value> = (1..current_turn)
        .map(|i| {
            serde_json::json!({
                "turn_id": i,
                "session_id": orch.session_id().to_string(),
                "status": "completed",
            })
        })
        .collect();

    success_response(serde_json::json!({
        "items": turns,
        "pagination": {
            "has_more": false,
            "total_count": turns.len(),
        }
    }))
    .into_response()
}

/// Execute a turn: simulate user speech → ASR → Agent → TTS → playback.
/// Uses mock adapters locally — no third-party services needed.
async fn execute_turn(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
    Json(req): Json<ExecuteTurnRequest>,
) -> impl IntoResponse {
    let session_id = match parse_session_id(&session_id_str) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_PARAMETER",
                "Invalid session_id",
            )
            .into_response();
        }
    };

    let orch_arc = {
        let sessions = state.sessions.read();
        match sessions.get(&session_id) {
            Some(arc) => Arc::clone(arc),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session does not exist",
                )
                .into_response();
            }
        }
    };

    let mut orch = orch_arc.lock().await;

    let user_text = req.text.clone().unwrap_or_default();

    // If text provided, pass it directly to Agent (skip ASR).
    // If no text, run ASR pipeline with dummy audio.
    let result = if !user_text.is_empty() {
        orch.execute_turn_with_text(None, Some(user_text.clone()))
            .await
    } else {
        orch.execute_turn(Some(vec![0u8; 320])).await
    };

    match result {
        Ok(response_text) => {
            let info = TurnResult {
                session_id: orch.session_id().to_string(),
                turn_id: orch.current_turn().as_u32() - 1,
                state: orch.state().to_string(),
                user_transcript: user_text,
                agent_response: response_text,
            };
            success_response(info).into_response()
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TURN_EXECUTION_FAILED",
            &e.to_string(),
        )
        .into_response(),
    }
}

async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.metrics.snapshot();
    let mut lines = Vec::new();

    // Counters
    for (name, value) in &snapshot.counters {
        lines.push(format!("# TYPE {name} counter"));
        lines.push(format!("{name} {value}"));
    }

    // Gauges
    for (name, value) in &snapshot.gauges {
        lines.push(format!("# TYPE {name} gauge"));
        lines.push(format!("{name} {value}"));
    }

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        lines.join("\n"),
    )
}

async fn list_audit(State(state): State<AppState>) -> impl IntoResponse {
    let records = state.audit.query(&AuditQuery {
        limit: Some(50),
        ..Default::default()
    });
    let items: Vec<serde_json::Value> = records
        .iter()
        .map(|r| {
            serde_json::json!({
                "audit_id": r.audit_id,
                "timestamp": r.timestamp.to_rfc3339(),
                "action": r.action,
                "target_type": r.target_type,
                "target_id": r.target_id,
                "result": r.result,
                "reason": r.reason,
            })
        })
        .collect();
    success_response(serde_json::json!({ "items": items, "total": items.len() })).into_response()
}

async fn billing_summary(State(state): State<AppState>) -> impl IntoResponse {
    success_response(serde_json::json!({
        "total_entries": state.billing.count(),
        "note": "Use tenant-specific endpoints for detailed billing"
    }))
    .into_response()
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn health_live() -> StatusCode {
    StatusCode::OK
}

async fn health_ready() -> StatusCode {
    StatusCode::OK
}

/// Parse session ID from the prefixed string format "sess-{uuid}".
fn parse_session_id(s: &str) -> Option<SessionId> {
    let uuid_str = s.strip_prefix("sess-")?;
    let uuid = Uuid::parse_str(uuid_str).ok()?;
    Some(SessionId::from_uuid(uuid))
}
