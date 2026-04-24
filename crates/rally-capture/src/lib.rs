#![deny(unsafe_code)]

pub mod ring;
pub mod source;

pub use ring::LineIndexedRing;
pub use source::{CaptureSnapshot, CaptureSource, DumpScreenSource};
