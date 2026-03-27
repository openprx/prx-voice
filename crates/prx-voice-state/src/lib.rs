//! PRX Voice session state machine.
//!
//! Implements the 12-state FSM from the state machine spec:
//! Idle → Connecting → Listening → UserSpeaking → AsrProcessing →
//! Thinking → Speaking → Interrupted → Paused → HandoffPending →
//! Closed / Failed

pub mod machine;
pub mod timer;
pub mod transition;
