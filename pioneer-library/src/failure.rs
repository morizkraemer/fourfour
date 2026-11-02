use std::fmt;

use rusqlite::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFailure {
    NewerVersion,
    Damaged,
    Locked,
    InvalidBackup,
    DiskFull,
    PermissionDenied,
}

#[derive(Debug)]
pub struct Failure {
    kind: LibraryFailure,
    message: String,
}

impl Failure {
    pub fn new(kind: LibraryFailure, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }

    pub fn error(kind: LibraryFailure, message: impl Into<String>) -> anyhow::Error {
        anyhow::Error::new(Self::new(kind, message))
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

fn sqlite_failure(code: ErrorCode) -> Option<LibraryFailure> {
    match code {
        ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => Some(LibraryFailure::Locked),
        ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt => Some(LibraryFailure::Damaged),
        ErrorCode::DiskFull => Some(LibraryFailure::DiskFull),
        ErrorCode::ReadOnly | ErrorCode::PermissionDenied => Some(LibraryFailure::PermissionDenied),
        _ => None,
    }
}

/// What the library knows about an error: its own marked failures first, then the SQLite error codes in the chain.
pub fn classify(err: &anyhow::Error) -> Option<LibraryFailure> {
    if let Some(failure) = err.downcast_ref::<Failure>() {
        return Some(failure.kind);
    }
    err.chain().find_map(|cause| match cause.downcast_ref::<rusqlite::Error>() {
        Some(rusqlite::Error::SqliteFailure(error, _)) => sqlite_failure(error.code),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sqlite_error(code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
    }

    #[test]
    fn a_marked_failure_is_found_through_context_layers() {
        let err = Failure::error(LibraryFailure::NewerVersion, "too new").context("one").context("two");
        assert_eq!(classify(&err), Some(LibraryFailure::NewerVersion));
        assert_eq!(err.to_string(), "two");
        assert_eq!(format!("{err:#}"), "two: one: too new");
    }

    #[test]
    fn a_marked_context_over_a_foreign_error_is_found_and_keeps_its_text() {
        let err = anyhow::Error::new(sqlite_error(rusqlite::ffi::SQLITE_ERROR)).context(Failure::new(LibraryFailure::Damaged, "unreadable"));
        assert_eq!(classify(&err), Some(LibraryFailure::Damaged));
        assert_eq!(err.to_string(), "unreadable");
    }

    #[test]
    fn sqlite_codes_map_to_failures() {
        let cases = [
            (rusqlite::ffi::SQLITE_BUSY, Some(LibraryFailure::Locked)),
            (rusqlite::ffi::SQLITE_LOCKED, Some(LibraryFailure::Locked)),
            (rusqlite::ffi::SQLITE_NOTADB, Some(LibraryFailure::Damaged)),
            (rusqlite::ffi::SQLITE_CORRUPT, Some(LibraryFailure::Damaged)),
            (rusqlite::ffi::SQLITE_FULL, Some(LibraryFailure::DiskFull)),
            (rusqlite::ffi::SQLITE_READONLY, Some(LibraryFailure::PermissionDenied)),
            (rusqlite::ffi::SQLITE_PERM, Some(LibraryFailure::PermissionDenied)),
            (rusqlite::ffi::SQLITE_ERROR, None),
        ];
        for (code, expected) in cases {
            let err = anyhow::Error::new(sqlite_error(code)).context("while opening");
            assert_eq!(classify(&err), expected, "code {code}");
        }
    }

    #[test]
    fn a_plain_error_is_unclassified() {
        assert_eq!(classify(&anyhow::anyhow!("nothing special")), None);
    }
}
