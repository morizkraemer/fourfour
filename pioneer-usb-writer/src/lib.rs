//! Pioneer CDJ USB format library.
//!
//! Pure format library for writing Pioneer CDJ-compatible USB drives.
//! No audio analysis is performed here — callers supply pre-analyzed
//! [`models::AnalysisResult`] values produced by an external analyzer
//! (e.g. `stratum-dsp`).
//!
//! # Supported output formats
//! - **PDB** (`export.pdb`) — the legacy DeviceSQL binary database read by all CDJ generations.
//! - **OneLibrary** (`exportLibrary.db`) — the SQLCipher-encrypted SQLite database required by CDJ-3000 and later.
//! - **ANLZ** (`ANLZ0000.DAT` / `ANLZ0000.EXT`) — per-track analysis files containing beat grids, waveforms, and cue points.
//!
//! # Pipeline
//! ```text
//! scanner::scan_directory()  →  Vec<Track>        (tag metadata)
//! analyzer::analyze_track()  →  AnalysisResult     (BPM/key — external crate)
//! writer::filesystem::write_usb()                  (writes all output)
//! reader::read_usb_state()   →  ExistingUsbState   (read back existing USB)
//! reader::masterdb::read_masterdb()  →  MasterDbImport  (import from Rekordbox)
//! ```

pub mod models;
pub mod reader;
pub mod scanner;
pub mod writer;

/// Application version — update on every release/edit.
pub const VERSION: &str = "0.9.21";
