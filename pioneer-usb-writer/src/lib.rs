#[cfg(feature = "analyzer")]
pub mod analyzer;
pub mod models;
pub mod scanner;
#[cfg(feature = "analyzer")]
pub mod waveform;
pub mod writer;

/// Application version — update on every release/edit.
pub const VERSION: &str = "0.5.1";
