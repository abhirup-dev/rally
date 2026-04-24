#![deny(unsafe_code)]
#![warn(missing_docs)]

//! Pure domain model for Rally — no IO, no async, no Zellij.

/// Agent state machine, triggers, and entity.
pub mod agent;
/// Capture mode and reference types.
pub mod capture;
/// Domain events emitted by state changes.
pub mod event;
/// Newtype IDs and timestamps.
pub mod ids;
/// Inbox items and urgency levels.
pub mod inbox;
/// Pane reference binding agents to Zellij panes.
pub mod pane;
/// Intent validation and access policy.
pub mod policy;
/// Repository and service port traits.
pub mod ports;
/// Workspace entity and canonical key generation.
pub mod workspace;
