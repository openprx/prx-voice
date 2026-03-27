//! PRX Voice session orchestrator.
//!
//! Owns the lifecycle of a single voice session, coordinating
//! the state machine, adapters, event emission, and turn management.

pub mod budget;
pub mod config;
pub mod handoff;
pub mod manager;
pub mod orchestrator;
pub mod recording;
