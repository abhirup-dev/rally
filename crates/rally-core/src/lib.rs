#![deny(unsafe_code)]
#![warn(missing_docs)]

//! Pure domain model for Rally — no IO, no async, no Zellij.

pub mod agent;
pub mod capture;
pub mod event;
pub mod ids;
pub mod inbox;
pub mod pane;
pub mod policy;
pub mod ports;
pub mod workspace;
