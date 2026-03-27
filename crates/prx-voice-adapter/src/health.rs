//! Adapter health status types.

use serde::{Deserialize, Serialize};

/// Adapter health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterStatus {
    Ready,
    Degraded,
    Down,
}

/// Adapter lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterLifecycle {
    Initializing,
    Warmup,
    Ready,
    Serving,
    Degraded,
    Draining,
    Stopped,
}

/// Health report from an adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: AdapterStatus,
    pub latency_ms: Option<u64>,
    pub error_rate_pct: Option<f64>,
    pub message: Option<String>,
}
