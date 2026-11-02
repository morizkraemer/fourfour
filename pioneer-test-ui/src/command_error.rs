use std::fmt;
use std::io;
use std::path::Path;

use pioneer_library::failure::{self, LibraryFailure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    LibraryNewerVersion,
    LibraryDamaged,
    LibraryLocked,
    NotFound,
    PermissionDenied,
    DiskFull,
    NoSpaceOnStick,
    DriveGone,
    StickBusy,
    DecodeFailed,
    AudioOutputUnavailable,
    ModelsMissing,
    AnalysisFailed,
    InvalidBackup,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl CommandError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), path: None }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Other, message)
    }

    pub fn with_path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_string_lossy().into_owned());
        self
    }

    /// A path that is missing on a stick command means the stick is gone.
    pub fn for_stick(mut self) -> Self {
        if self.kind == ErrorKind::NotFound {
            self.kind = ErrorKind::DriveGone;
        }
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn from_io(err: &io::Error, message: impl Into<String>) -> Self {
        Self::new(kind_of_io(err), message)
    }

    /// The message is the outermost context only, like `err.to_string()`.
    pub fn from_anyhow(err: anyhow::Error) -> Self {
        Self::new(kind_of_chain(&err), err.to_string())
    }

    /// The message is the whole chain, like `format!("{err:#}")`.
    pub fn from_anyhow_chain(err: anyhow::Error) -> Self {
        Self::new(kind_of_chain(&err), format!("{err:#}"))
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::other(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::other(message)
    }
}

#[cfg(unix)]
const ENODEV: i32 = 19;
#[cfg(unix)]
const ENXIO: i32 = 6;
#[cfg(unix)]
const EIO: i32 = 5;
#[cfg(unix)]
const ENOSPC: i32 = 28;

pub fn kind_of_io(err: &io::Error) -> ErrorKind {
    match err.kind() {
        io::ErrorKind::NotFound => ErrorKind::NotFound,
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => ErrorKind::PermissionDenied,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => ErrorKind::DiskFull,
        _ => kind_of_errno(err.raw_os_error()),
    }
}

#[cfg(unix)]
fn kind_of_errno(errno: Option<i32>) -> ErrorKind {
    match errno {
        Some(ENOSPC) => ErrorKind::DiskFull,
        Some(ENODEV | ENXIO | EIO) => ErrorKind::DriveGone,
        _ => ErrorKind::Other,
    }
}

#[cfg(not(unix))]
fn kind_of_errno(_errno: Option<i32>) -> ErrorKind {
    ErrorKind::Other
}

fn kind_of_library_failure(failure: LibraryFailure) -> ErrorKind {
    match failure {
        LibraryFailure::NewerVersion => ErrorKind::LibraryNewerVersion,
        LibraryFailure::Damaged => ErrorKind::LibraryDamaged,
        LibraryFailure::Locked => ErrorKind::LibraryLocked,
        LibraryFailure::InvalidBackup => ErrorKind::InvalidBackup,
        LibraryFailure::DiskFull => ErrorKind::DiskFull,
        LibraryFailure::PermissionDenied => ErrorKind::PermissionDenied,
    }
}

pub fn kind_of_chain(err: &anyhow::Error) -> ErrorKind {
    if let Some(failure) = failure::classify(err) {
        return kind_of_library_failure(failure);
    }
    err.chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
        .map_or(ErrorKind::Other, kind_of_io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use serde_json::json;

    fn io_error(kind: io::ErrorKind) -> io::Error {
        io::Error::new(kind, "boom")
    }

    #[test]
    fn it_serialises_to_the_wire_shape() {
        let error = CommandError::new(ErrorKind::LibraryNewerVersion, "too new").with_path("/lib/library.db");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "kind": "library_newer_version", "message": "too new", "path": "/lib/library.db" })
        );
        assert_eq!(
            serde_json::to_value(CommandError::other("x")).unwrap(),
            json!({ "kind": "other", "message": "x" })
        );
    }

    #[test]
    fn every_kind_has_a_snake_case_wire_name() {
        let names = [
            (ErrorKind::LibraryNewerVersion, "library_newer_version"),
            (ErrorKind::LibraryDamaged, "library_damaged"),
            (ErrorKind::LibraryLocked, "library_locked"),
            (ErrorKind::NotFound, "not_found"),
            (ErrorKind::PermissionDenied, "permission_denied"),
            (ErrorKind::DiskFull, "disk_full"),
            (ErrorKind::NoSpaceOnStick, "no_space_on_stick"),
            (ErrorKind::DriveGone, "drive_gone"),
            (ErrorKind::StickBusy, "stick_busy"),
            (ErrorKind::DecodeFailed, "decode_failed"),
            (ErrorKind::AudioOutputUnavailable, "audio_output_unavailable"),
            (ErrorKind::ModelsMissing, "models_missing"),
            (ErrorKind::AnalysisFailed, "analysis_failed"),
            (ErrorKind::InvalidBackup, "invalid_backup"),
            (ErrorKind::Other, "other"),
        ];
        for (kind, name) in names {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(name));
        }
    }

    #[test]
    fn a_plain_string_is_other_and_keeps_its_text() {
        let error = CommandError::from("The library is not open".to_string());
        assert_eq!(error.kind, ErrorKind::Other);
        assert_eq!(error.to_string(), "The library is not open");
    }

    #[test]
    fn io_error_kinds_map_to_kinds() {
        assert_eq!(kind_of_io(&io_error(io::ErrorKind::NotFound)), ErrorKind::NotFound);
        assert_eq!(kind_of_io(&io_error(io::ErrorKind::PermissionDenied)), ErrorKind::PermissionDenied);
        assert_eq!(kind_of_io(&io_error(io::ErrorKind::ReadOnlyFilesystem)), ErrorKind::PermissionDenied);
        assert_eq!(kind_of_io(&io_error(io::ErrorKind::StorageFull)), ErrorKind::DiskFull);
        assert_eq!(kind_of_io(&io_error(io::ErrorKind::InvalidData)), ErrorKind::Other);
    }

    #[cfg(unix)]
    #[test]
    fn os_errors_for_a_vanished_drive_and_a_full_disk_map_to_kinds() {
        assert_eq!(kind_of_io(&io::Error::from_raw_os_error(ENOSPC)), ErrorKind::DiskFull);
        assert_eq!(kind_of_io(&io::Error::from_raw_os_error(EIO)), ErrorKind::DriveGone);
        assert_eq!(kind_of_io(&io::Error::from_raw_os_error(ENODEV)), ErrorKind::DriveGone);
        assert_eq!(kind_of_io(&io::Error::from_raw_os_error(ENXIO)), ErrorKind::DriveGone);
    }

    #[test]
    fn an_io_error_under_context_layers_is_found() {
        let err = Err::<(), _>(io_error(io::ErrorKind::PermissionDenied))
            .context("Failed to create /x/y.partial")
            .context("Backup failed")
            .unwrap_err();
        assert_eq!(kind_of_chain(&err), ErrorKind::PermissionDenied);
    }

    #[test]
    fn from_anyhow_keeps_the_text_of_to_string_and_of_the_alternate_format() {
        let make = || Err::<(), _>(io_error(io::ErrorKind::NotFound)).context("Failed to open the file").unwrap_err();
        let top = CommandError::from_anyhow(make());
        let chain = CommandError::from_anyhow_chain(make());
        assert_eq!((top.kind, top.message.as_str()), (ErrorKind::NotFound, "Failed to open the file"));
        assert_eq!((chain.kind, chain.message.as_str()), (ErrorKind::NotFound, "Failed to open the file: boom"));
    }

    #[test]
    fn a_library_failure_wins_over_an_io_error_below_it() {
        let err = anyhow::Error::new(io_error(io::ErrorKind::NotFound))
            .context(failure::Failure::new(LibraryFailure::Damaged, "unreadable"));
        assert_eq!(kind_of_chain(&err), ErrorKind::LibraryDamaged);
    }

    #[test]
    fn library_failures_map_to_kinds() {
        let cases = [
            (LibraryFailure::NewerVersion, ErrorKind::LibraryNewerVersion),
            (LibraryFailure::Damaged, ErrorKind::LibraryDamaged),
            (LibraryFailure::Locked, ErrorKind::LibraryLocked),
            (LibraryFailure::InvalidBackup, ErrorKind::InvalidBackup),
            (LibraryFailure::DiskFull, ErrorKind::DiskFull),
            (LibraryFailure::PermissionDenied, ErrorKind::PermissionDenied),
        ];
        for (failure, expected) in cases {
            let err = failure::Failure::error(failure, "x");
            assert_eq!(kind_of_chain(&err), expected);
        }
    }

    #[test]
    fn a_missing_path_on_a_stick_command_is_a_stick_that_is_gone() {
        assert_eq!(CommandError::new(ErrorKind::NotFound, "x").for_stick().kind, ErrorKind::DriveGone);
        assert_eq!(CommandError::new(ErrorKind::PermissionDenied, "x").for_stick().kind, ErrorKind::PermissionDenied);
    }

    #[test]
    fn an_unknown_error_is_other() {
        assert_eq!(kind_of_chain(&anyhow::anyhow!("something else")), ErrorKind::Other);
    }
}
