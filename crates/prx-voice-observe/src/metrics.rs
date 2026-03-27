//! In-memory metrics registry.
//!
//! Tracks session, turn, vendor, and resource metrics
//! per the production plan observability requirements.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global metrics registry.
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, Arc<AtomicU64>>>,
    gauges: RwLock<HashMap<String, Arc<AtomicU64>>>,
    histograms: RwLock<HashMap<String, Arc<RwLock<Vec<f64>>>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
        }
    }

    /// Increment a counter by 1.
    pub fn inc(&self, name: &str) {
        self.inc_by(name, 1);
    }

    /// Increment a counter by a value.
    pub fn inc_by(&self, name: &str, val: u64) {
        let counters = self.counters.read();
        if let Some(c) = counters.get(name) {
            c.fetch_add(val, Ordering::Relaxed);
            return;
        }
        drop(counters);

        let c = Arc::new(AtomicU64::new(val));
        self.counters.write().entry(name.to_string()).or_insert(c);
    }

    /// Get counter value.
    pub fn counter(&self, name: &str) -> u64 {
        self.counters
            .read()
            .get(name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Set a gauge value.
    pub fn gauge_set(&self, name: &str, val: u64) {
        let gauges = self.gauges.read();
        if let Some(g) = gauges.get(name) {
            g.store(val, Ordering::Relaxed);
            return;
        }
        drop(gauges);

        self.gauges
            .write()
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(val)));
    }

    /// Increment a gauge by 1.
    pub fn gauge_inc(&self, name: &str) {
        let gauges = self.gauges.read();
        if let Some(g) = gauges.get(name) {
            g.fetch_add(1, Ordering::Relaxed);
            return;
        }
        drop(gauges);

        self.gauges
            .write()
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(1)));
    }

    /// Decrement a gauge by 1.
    pub fn gauge_dec(&self, name: &str) {
        let gauges = self.gauges.read();
        if let Some(g) = gauges.get(name) {
            g.fetch_sub(1, Ordering::Relaxed);
            return;
        }
        drop(gauges);
        // Don't create a gauge just to decrement
    }

    /// Get gauge value.
    pub fn gauge(&self, name: &str) -> u64 {
        self.gauges
            .read()
            .get(name)
            .map(|g| g.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Record a histogram observation.
    pub fn observe(&self, name: &str, val: f64) {
        let histograms = self.histograms.read();
        if let Some(h) = histograms.get(name) {
            h.write().push(val);
            return;
        }
        drop(histograms);

        let h = Arc::new(RwLock::new(vec![val]));
        self.histograms.write().entry(name.to_string()).or_insert(h);
    }

    /// Get histogram p95 (approximate).
    pub fn histogram_p95(&self, name: &str) -> Option<f64> {
        let histograms = self.histograms.read();
        let h = histograms.get(name)?;
        let mut values = h.read().clone();
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((values.len() as f64) * 0.95).ceil() as usize;
        let idx = idx.min(values.len()) - 1;
        Some(values[idx])
    }

    /// Export all metrics as a snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let counters: HashMap<String, u64> = self
            .counters
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect();
        let gauges: HashMap<String, u64> = self
            .gauges
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect();
        MetricsSnapshot { counters, gauges }
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of all counter and gauge metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, u64>,
}

/// Well-known metric names (from production plan).
pub mod metric_names {
    // Session metrics
    pub const ACTIVE_SESSIONS: &str = "prx_voice_active_sessions";
    pub const SESSION_CREATED_TOTAL: &str = "prx_voice_session_created_total";
    pub const SESSION_CLOSED_TOTAL: &str = "prx_voice_session_closed_total";
    pub const SESSION_FAILED_TOTAL: &str = "prx_voice_session_failed_total";
    pub const INTERRUPTS_TOTAL: &str = "prx_voice_interrupts_total";

    // Turn latency
    pub const TURN_LATENCY_MS: &str = "prx_voice_turn_latency_ms";
    pub const ASR_FINAL_LATENCY_MS: &str = "prx_voice_asr_final_latency_ms";
    pub const AGENT_FIRST_TOKEN_LATENCY_MS: &str = "prx_voice_agent_first_token_latency_ms";
    pub const TTS_FIRST_CHUNK_LATENCY_MS: &str = "prx_voice_tts_first_chunk_latency_ms";

    // Vendor metrics
    pub const PROVIDER_ERRORS_TOTAL: &str = "prx_voice_provider_errors_total";
    pub const PROVIDER_FALLBACK_TOTAL: &str = "prx_voice_provider_fallback_total";

    // Resource metrics
    pub const DROPPED_FRAMES_TOTAL: &str = "prx_voice_dropped_frames_total";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_increments() {
        let m = MetricsRegistry::new();
        m.inc("test_counter");
        m.inc("test_counter");
        assert_eq!(m.counter("test_counter"), 2);
    }

    #[test]
    fn gauge_set_and_read() {
        let m = MetricsRegistry::new();
        m.gauge_set("test_gauge", 42);
        assert_eq!(m.gauge("test_gauge"), 42);
        m.gauge_set("test_gauge", 10);
        assert_eq!(m.gauge("test_gauge"), 10);
    }

    #[test]
    fn gauge_inc_dec() {
        let m = MetricsRegistry::new();
        m.gauge_inc("sessions");
        m.gauge_inc("sessions");
        m.gauge_inc("sessions");
        m.gauge_dec("sessions");
        assert_eq!(m.gauge("sessions"), 2);
    }

    #[test]
    fn histogram_p95() {
        let m = MetricsRegistry::new();
        for i in 1..=100 {
            m.observe("latency", i as f64);
        }
        let p95 = m.histogram_p95("latency").unwrap();
        assert!(p95 >= 95.0);
    }

    #[test]
    fn snapshot_contains_all() {
        let m = MetricsRegistry::new();
        m.inc("c1");
        m.gauge_set("g1", 5);
        let snap = m.snapshot();
        assert_eq!(snap.counters["c1"], 1);
        assert_eq!(snap.gauges["g1"], 5);
    }

    #[test]
    fn unknown_metrics_return_zero() {
        let m = MetricsRegistry::new();
        assert_eq!(m.counter("nonexistent"), 0);
        assert_eq!(m.gauge("nonexistent"), 0);
        assert!(m.histogram_p95("nonexistent").is_none());
    }
}
