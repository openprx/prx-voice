# PRX Voice Engine

A real-time voice conversation orchestration engine built in Rust. PRX Voice manages the full lifecycle of AI-powered voice sessions — from speech recognition to agent reasoning to speech synthesis — with enterprise-grade observability, multi-tenancy, and compliance built in.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   prx-voice-bin                     │  ← Entry point
├─────────────────────────────────────────────────────┤
│              prx-voice-control                      │  ← REST / gRPC / WebSocket API
│         (auth, jwt, ratelimit, routing)              │
├──────────────┬──────────────┬───────────────────────┤
│  session     │   adapter    │   transport           │
│ (orchestrator│ (asr, tts,   │  (websocket,          │
│  manager,    │  agent, vad, │   media channel)      │
│  handoff,    │  fallback)   │                       │
│  recording)  │              │                       │
├──────────────┴──────────────┴───────────────────────┤
│   state (FSM)  │  event (bus, envelope, replay)     │
├────────────────┴────────────────────────────────────┤
│  policy    │  billing   │  audit     │  observe     │
│ (rbac,     │ (ledger,   │ (record,   │ (metrics,    │
│  tenant,   │  meter,    │  store,    │  slo,        │
│  quota)    │  pricing)  │  compliance│  degradation)│
├─────────────────────────────────────────────────────┤
│  storage (postgres, memory, object, migrations)     │
├─────────────────────────────────────────────────────┤
│  core (settings, flags, security, deploy)           │
├─────────────────────────────────────────────────────┤
│  types (ids, error, redact)                         │  ← Zero-dependency leaf
└─────────────────────────────────────────────────────┘
```

### Crate Overview

| Crate | Purpose |
|-------|---------|
| **prx-voice-types** | Shared IDs, error codes, enums. Zero external dependencies — leaf crate for the entire workspace. |
| **prx-voice-state** | 12-state session FSM (Idle → Listening → UserSpeaking → AsrProcessing → Thinking → Speaking → …) with transition rules and interrupt semantics. |
| **prx-voice-event** | CloudEvents v1.0 event system with bus, envelope, payload definitions, and event replay. |
| **prx-voice-adapter** | Trait-based adapter interfaces for ASR, TTS, Agent, and VAD. Ships with mock, Deepgram, Azure, OpenAI, and local Sherpa/Ollama implementations. |
| **prx-voice-session** | Session orchestrator coordinating state machine, adapters, event emission, turn management, handoff, and recording. |
| **prx-voice-transport** | WebSocket and media channel abstraction. |
| **prx-voice-control** | HTTP/gRPC control plane — REST API, JWT authentication, rate limiting. |
| **prx-voice-policy** | Multi-tenant isolation, RBAC permission model, quota enforcement. |
| **prx-voice-billing** | Usage metering, ledger, and pricing engine. |
| **prx-voice-audit** | Audit logging, compliance checks, and audit record storage. |
| **prx-voice-observe** | Prometheus metrics, SLO monitoring, 4-level degradation strategy, incident management. |
| **prx-voice-storage** | PostgreSQL, in-memory, and object storage backends with migration support. |
| **prx-voice-core** | Global configuration, feature flags, security settings, deployment config. |
| **prx-voice-bin** | Server binary entry point with graceful shutdown. |

## Getting Started

### Prerequisites

- Rust 1.85+ (Edition 2024)
- PostgreSQL 15+ (optional, in-memory store available for development)

### Build

```bash
cargo build --workspace
```

### Run

```bash
# With default config (mock adapters, in-memory store)
cargo run -p prx-voice-bin

# With custom config
PRX_VOICE_HOST=0.0.0.0 PRX_VOICE_PORT=3000 cargo run -p prx-voice-bin

# Override specific settings via environment variables
PRX_VOICE_SERVER__PORT=8080 cargo run -p prx-voice-bin
```

The server starts on `http://localhost:3000` by default.

### Docker

```bash
# Build and run
docker compose up --build

# Or build image directly
docker build -t prx-voice .
docker run -p 3000:3000 prx-voice
```

### Test

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p prx-voice-state
cargo test -p prx-voice-session

# Integration tests
cargo test -p prx-voice-integration-tests
```

## Configuration

Configuration is loaded from `config.yaml` and can be overridden via environment variables with the `PRX_VOICE_` prefix:

```yaml
server:
  host: "0.0.0.0"
  port: 3000

session:
  max_duration_sec: 1800
  max_turns: 100
  interrupt_enabled: true
  default_language: "en-US"

adapters:
  default_asr_provider: "mock"     # mock | deepgram | local
  default_agent_provider: "mock"   # mock | openai | local
  default_tts_provider: "mock"     # mock | azure | local
```

### Adapter Providers

| Component | Provider | Description |
|-----------|----------|-------------|
| ASR | `mock` | Returns canned transcriptions for testing |
| ASR | `deepgram` | Deepgram streaming ASR (`DEEPGRAM_API_KEY`) |
| ASR | `local` | Sherpa-ONNX offline ASR |
| Agent | `mock` | Echo agent for testing |
| Agent | `openai` | OpenAI GPT models (`OPENAI_API_KEY`) |
| Agent | `local` | Ollama local LLM |
| TTS | `mock` | Returns silence for testing |
| TTS | `azure` | Azure Cognitive Services TTS (`AZURE_SPEECH_KEY`, `AZURE_SPEECH_REGION`) |
| TTS | `local` | Sherpa-ONNX offline TTS |

## API

### REST Endpoints

```
POST   /api/v1/sessions              Create a new voice session
GET    /api/v1/sessions/:id          Get session details
POST   /api/v1/sessions/:id/end      End a session
POST   /api/v1/sessions/:id/interrupt Interrupt current playback
GET    /api/v1/sessions/:id/events   SSE event stream
GET    /api/v1/health/live           Liveness probe
GET    /api/v1/health/ready          Readiness probe
GET    /api/v1/metrics               Prometheus metrics
```

### WebSocket

```
GET    /api/v1/ws/:session_id        Full-duplex audio + events
```

All REST responses use a unified envelope:

```json
{
  "request_id": "uuid",
  "timestamp": "2024-01-01T00:00:00Z",
  "data": { ... },
  "error": null
}
```

## Deployment

### Kubernetes (Helm)

```bash
helm install prx-voice deploy/helm/prx-voice \
  -f deploy/helm/values-us-east.yaml
```

Region-specific value files are provided for `us-east`, `us-west`, and `eu-west`.

## Project Status

This project is in **alpha**. The core session lifecycle, state machine, event system, and adapter framework are functional. Enterprise features (billing, audit, RBAC) are structurally complete but not yet production-hardened.

## License

MIT License — see [LICENSE](LICENSE) for details.
