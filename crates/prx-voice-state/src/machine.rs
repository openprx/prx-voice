//! The core session state machine.
//!
//! Implements all transition rules from the state machine spec.
//! Rules:
//! 1. Only one primary response flow at a time.
//! 2. User speaks during Speaking → must enter Interrupt path.
//! 3. TranscriptFinal is prerequisite for Thinking.
//! 4. Old turn's pending segments must be flushed on interrupt.
//! 5. Failed state must never silently swallow errors.

use crate::transition::{SessionState, TransitionResult, Trigger};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Configuration for the state machine (per session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineConfig {
    /// Whether barge-in/interrupt is enabled.
    pub interrupt_enabled: bool,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            interrupt_enabled: true,
        }
    }
}

/// The session state machine.
#[derive(Debug)]
pub struct SessionStateMachine {
    state: SessionState,
    config: StateMachineConfig,
    transition_count: u64,
}

impl SessionStateMachine {
    /// Create a new state machine in Idle state.
    pub fn new(config: StateMachineConfig) -> Self {
        Self {
            state: SessionState::Idle,
            config,
            transition_count: 0,
        }
    }

    /// Current state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Number of transitions that have occurred.
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    /// Attempt a state transition. Returns the result.
    pub fn apply(&mut self, trigger: Trigger) -> TransitionResult {
        if self.state.is_terminal() {
            return TransitionResult::AlreadyTerminal {
                current: self.state,
            };
        }

        let result = self.compute_transition(&trigger);

        if let TransitionResult::Transitioned { from, to, .. } = &result {
            info!(
                from = %from,
                to = %to,
                trigger = ?trigger,
                transition_count = self.transition_count + 1,
                "state transition"
            );
            self.state = *to;
            self.transition_count += 1;
        }

        result
    }

    fn compute_transition(&self, trigger: &Trigger) -> TransitionResult {
        use SessionState::*;
        use Trigger::*;

        let from = self.state;

        // Global transitions: any non-terminal state → Failed
        match trigger {
            TransportDisconnect | ResourceExhaustion | UnrecoverableError { .. } => {
                return TransitionResult::Transitioned {
                    from,
                    to: Failed,
                    trigger: trigger.clone(),
                };
            }
            _ => {}
        }

        // Global: close request from any non-terminal state
        if let CloseRequest { force: _ } = trigger {
            return TransitionResult::Transitioned {
                from,
                to: Closed,
                trigger: trigger.clone(),
            };
        }

        // Global: pause from pausable states
        if let PauseRequest = trigger {
            if from.is_pausable() {
                return TransitionResult::Transitioned {
                    from,
                    to: Paused,
                    trigger: trigger.clone(),
                };
            }
            return TransitionResult::Rejected {
                current: from,
                trigger: trigger.clone(),
                reason: format!("Cannot pause from {from}: not a pausable state"),
            };
        }

        // Global: handoff from handoffable states
        if let HandoffRequest = trigger {
            if from.is_handoffable() {
                return TransitionResult::Transitioned {
                    from,
                    to: HandoffPending,
                    trigger: trigger.clone(),
                };
            }
            return TransitionResult::Rejected {
                current: from,
                trigger: trigger.clone(),
                reason: format!("Cannot handoff from {from}: not a handoffable state"),
            };
        }

        // State-specific transitions
        let next = match (from, trigger) {
            // Idle
            (Idle, SessionCreate) => Some(Connecting),
            (Idle, Timeout { .. }) => Some(Closed),

            // Connecting
            (Connecting, AdaptersReady) => Some(Listening),
            (Connecting, AdapterInitFailed | AdapterInitTimeout) => Some(Failed),
            (Connecting, Timeout { .. }) => Some(Failed),

            // Listening
            (Listening, VadStarted) => Some(UserSpeaking),
            (Listening, Timeout { .. }) => Some(Closed), // idle_timeout

            // UserSpeaking
            (UserSpeaking, VadEnded) => Some(AsrProcessing),
            (UserSpeaking, Timeout { .. }) => Some(AsrProcessing), // max_utterance_timeout → force VadEnded

            // AsrProcessing
            (AsrProcessing, TranscriptFinal { is_empty: false }) => Some(Thinking),
            (AsrProcessing, TranscriptFinal { is_empty: true }) => Some(Listening), // empty → rollback
            (AsrProcessing, AsrTimeout) => Some(Failed),
            (AsrProcessing, Timeout { .. }) => Some(Failed),

            // Thinking
            (Thinking, AgentResponseStart) => Some(Speaking),
            (Thinking, AgentTimeout) => Some(Failed),
            (Thinking, AgentHandoffDecision) => Some(HandoffPending),
            (Thinking, Timeout { .. }) => Some(Failed),

            // Speaking
            (Speaking, PlaybackCompleted) => Some(Listening), // turn loop
            (Speaking, InterruptDetected) if self.config.interrupt_enabled => Some(Interrupted),
            (Speaking, InterruptDetected) => {
                return TransitionResult::Rejected {
                    current: from,
                    trigger: trigger.clone(),
                    reason: "Interrupt disabled by config".into(),
                };
            }
            (Speaking, TtsError) => Some(Failed),
            (Speaking, Timeout { .. }) => Some(Listening), // max TTS timeout → force stop

            // Interrupted
            (Interrupted, InterruptResolved) => Some(Listening),
            (Interrupted, Timeout { .. }) => Some(Listening), // 5s force clear

            // Paused
            (Paused, ResumeRequest) => Some(Listening),
            (Paused, Timeout { .. }) => Some(Closed), // pause_timeout
            (Paused, HandoffTimeout) => Some(Closed),

            // HandoffPending
            (HandoffPending, HandoffConfirmed) => Some(Closed),
            (HandoffPending, HandoffTimeout) => Some(Closed),
            (HandoffPending, Timeout { .. }) => Some(Closed),

            _ => None,
        };

        match next {
            Some(to) => TransitionResult::Transitioned {
                from,
                to,
                trigger: trigger.clone(),
            },
            None => TransitionResult::Rejected {
                current: from,
                trigger: trigger.clone(),
                reason: format!("No valid transition from {from} for trigger {trigger:?}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_machine() -> SessionStateMachine {
        SessionStateMachine::new(StateMachineConfig::default())
    }

    // === Happy path: full turn ===

    #[test]
    fn happy_path_full_turn() {
        let mut sm = default_machine();
        assert_eq!(sm.state(), SessionState::Idle);

        assert!(sm.apply(Trigger::SessionCreate).is_success());
        assert_eq!(sm.state(), SessionState::Connecting);

        assert!(sm.apply(Trigger::AdaptersReady).is_success());
        assert_eq!(sm.state(), SessionState::Listening);

        assert!(sm.apply(Trigger::VadStarted).is_success());
        assert_eq!(sm.state(), SessionState::UserSpeaking);

        assert!(sm.apply(Trigger::VadEnded).is_success());
        assert_eq!(sm.state(), SessionState::AsrProcessing);

        assert!(
            sm.apply(Trigger::TranscriptFinal { is_empty: false })
                .is_success()
        );
        assert_eq!(sm.state(), SessionState::Thinking);

        assert!(sm.apply(Trigger::AgentResponseStart).is_success());
        assert_eq!(sm.state(), SessionState::Speaking);

        assert!(sm.apply(Trigger::PlaybackCompleted).is_success());
        assert_eq!(sm.state(), SessionState::Listening);

        assert_eq!(sm.transition_count(), 7);
    }

    // === Interrupt flow ===

    #[test]
    fn interrupt_during_speaking() {
        let mut sm = default_machine();
        // Advance to Speaking
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::VadStarted);
        sm.apply(Trigger::VadEnded);
        sm.apply(Trigger::TranscriptFinal { is_empty: false });
        sm.apply(Trigger::AgentResponseStart);
        assert_eq!(sm.state(), SessionState::Speaking);

        // Interrupt
        assert!(sm.apply(Trigger::InterruptDetected).is_success());
        assert_eq!(sm.state(), SessionState::Interrupted);

        // Resolve
        assert!(sm.apply(Trigger::InterruptResolved).is_success());
        assert_eq!(sm.state(), SessionState::Listening);
    }

    #[test]
    fn interrupt_disabled_rejects() {
        let mut sm = SessionStateMachine::new(StateMachineConfig {
            interrupt_enabled: false,
        });
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::VadStarted);
        sm.apply(Trigger::VadEnded);
        sm.apply(Trigger::TranscriptFinal { is_empty: false });
        sm.apply(Trigger::AgentResponseStart);

        let result = sm.apply(Trigger::InterruptDetected);
        assert!(!result.is_success());
        assert_eq!(sm.state(), SessionState::Speaking);
    }

    // === Empty transcript rollback ===

    #[test]
    fn empty_transcript_returns_to_listening() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::VadStarted);
        sm.apply(Trigger::VadEnded);

        assert!(
            sm.apply(Trigger::TranscriptFinal { is_empty: true })
                .is_success()
        );
        assert_eq!(sm.state(), SessionState::Listening);
    }

    // === Pause / Resume ===

    #[test]
    fn pause_from_listening() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        assert_eq!(sm.state(), SessionState::Listening);

        assert!(sm.apply(Trigger::PauseRequest).is_success());
        assert_eq!(sm.state(), SessionState::Paused);

        assert!(sm.apply(Trigger::ResumeRequest).is_success());
        assert_eq!(sm.state(), SessionState::Listening);
    }

    #[test]
    fn pause_from_non_pausable_rejects() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        assert_eq!(sm.state(), SessionState::Connecting);

        let result = sm.apply(Trigger::PauseRequest);
        assert!(!result.is_success());
    }

    // === Handoff ===

    #[test]
    fn handoff_from_listening() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);

        assert!(sm.apply(Trigger::HandoffRequest).is_success());
        assert_eq!(sm.state(), SessionState::HandoffPending);

        assert!(sm.apply(Trigger::HandoffConfirmed).is_success());
        assert_eq!(sm.state(), SessionState::Closed);
    }

    #[test]
    fn agent_decides_handoff_from_thinking() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::VadStarted);
        sm.apply(Trigger::VadEnded);
        sm.apply(Trigger::TranscriptFinal { is_empty: false });

        assert!(sm.apply(Trigger::AgentHandoffDecision).is_success());
        assert_eq!(sm.state(), SessionState::HandoffPending);
    }

    // === Failure paths ===

    #[test]
    fn adapter_init_failure() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        assert!(sm.apply(Trigger::AdapterInitFailed).is_success());
        assert_eq!(sm.state(), SessionState::Failed);
        assert!(sm.state().is_terminal());
    }

    #[test]
    fn transport_disconnect_from_any_state() {
        for start_triggers in [
            vec![Trigger::SessionCreate],
            vec![Trigger::SessionCreate, Trigger::AdaptersReady],
            vec![
                Trigger::SessionCreate,
                Trigger::AdaptersReady,
                Trigger::VadStarted,
            ],
        ] {
            let mut sm = default_machine();
            for t in start_triggers {
                sm.apply(t);
            }
            let prev = sm.state();
            assert!(
                sm.apply(Trigger::TransportDisconnect).is_success(),
                "TransportDisconnect should work from {prev}"
            );
            assert_eq!(sm.state(), SessionState::Failed);
        }
    }

    #[test]
    fn terminal_state_rejects_all() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::CloseRequest { force: false });
        assert_eq!(sm.state(), SessionState::Closed);

        let result = sm.apply(Trigger::VadStarted);
        assert!(matches!(result, TransitionResult::AlreadyTerminal { .. }));
    }

    // === Close from any state ===

    #[test]
    fn close_from_speaking() {
        let mut sm = default_machine();
        sm.apply(Trigger::SessionCreate);
        sm.apply(Trigger::AdaptersReady);
        sm.apply(Trigger::VadStarted);
        sm.apply(Trigger::VadEnded);
        sm.apply(Trigger::TranscriptFinal { is_empty: false });
        sm.apply(Trigger::AgentResponseStart);

        assert!(
            sm.apply(Trigger::CloseRequest { force: false })
                .is_success()
        );
        assert_eq!(sm.state(), SessionState::Closed);
    }
}
