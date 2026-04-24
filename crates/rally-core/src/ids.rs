use ulid::Ulid;

macro_rules! id_newtype {
    ($(#[doc = $doc:expr])? $name:ident) => {
        $(#[doc = $doc])?
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(transparent))]
        pub struct $name(Ulid);

        impl $name {
            /// Wrap a raw ULID.
            pub fn new(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// Unwrap to the inner ULID.
            pub fn inner(self) -> Ulid {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_newtype!(
    /// Unique workspace identifier.
    WorkspaceId
);
id_newtype!(
    /// Unique agent identifier.
    AgentId
);
id_newtype!(
    /// Unique pane identifier (domain-level).
    PaneId
);
id_newtype!(
    /// Unique inbox item identifier.
    InboxItemId
);
id_newtype!(
    /// Unique hook registration identifier.
    HookId
);

/// Unix milliseconds since epoch. `Copy` and totally ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Timestamp(u64);

impl Timestamp {
    /// Create from raw milliseconds since Unix epoch.
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    /// Extract the raw millisecond value.
    pub fn as_millis(self) -> u64 {
        self.0
    }
}
