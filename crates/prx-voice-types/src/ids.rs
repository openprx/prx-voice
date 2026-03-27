//! Strongly-typed identifiers for all PRX Voice entities.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Macro to generate a newtype ID wrapper over UUID.
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new random ID.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create from an existing UUID.
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Return the inner UUID.
            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Return the prefixed string representation.
            pub fn to_prefixed_string(&self) -> String {
                format!("{}-{}", $prefix, self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

define_id!(
    /// Unique tenant identifier.
    TenantId, "tenant"
);
define_id!(
    /// Unique workspace identifier.
    WorkspaceId, "ws"
);
define_id!(
    /// Unique project identifier.
    ProjectId, "proj"
);
define_id!(
    /// Unique session identifier.
    SessionId, "sess"
);
define_id!(
    /// Unique event identifier.
    EventId, "evt"
);
define_id!(
    /// Distributed trace identifier.
    TraceId, "trace"
);
define_id!(
    /// Span identifier within a trace.
    SpanId, "span"
);
define_id!(
    /// Unique speech utterance identifier.
    SpeechId, "utt"
);

/// Turn identifier — monotonically increasing integer within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TurnId(u32);

impl TurnId {
    /// First turn in a session.
    pub fn first() -> Self {
        Self(1)
    }

    /// Advance to next turn.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Return the numeric value.
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn-{}", self.0)
    }
}

/// Segment identifier — `seg-{turn_id}.{index}` format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentId {
    pub turn: TurnId,
    pub index: u32,
}

impl SegmentId {
    /// Create a new segment ID.
    pub fn new(turn: TurnId, index: u32) -> Self {
        Self { turn, index }
    }
}

impl fmt::Display for SegmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "seg-{}.{}", self.turn.as_u32(), self.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_display_has_prefix() {
        let id = SessionId::new();
        let s = id.to_string();
        assert!(s.starts_with("sess-"), "Expected prefix 'sess-', got: {s}");
    }

    #[test]
    fn tenant_id_serializes_as_uuid() {
        let id = TenantId::new();
        let json = serde_json::to_string(&id).unwrap();
        // Should serialize as plain UUID string (serde(transparent))
        assert!(json.starts_with('"'));
        assert!(!json.contains("tenant"));
    }

    #[test]
    fn turn_id_increments() {
        let t1 = TurnId::first();
        assert_eq!(t1.as_u32(), 1);
        let t2 = t1.next();
        assert_eq!(t2.as_u32(), 2);
    }

    #[test]
    fn segment_id_format() {
        let seg = SegmentId::new(TurnId::first(), 3);
        assert_eq!(seg.to_string(), "seg-1.3");
    }
}
