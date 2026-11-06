use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use crate::LocalLibrary;

static INSTANCES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn next_instance() -> u64 {
    INSTANCES.fetch_add(1, Ordering::Relaxed)
}

/// Equal revisions of one library mean that no row was written and no table changed in between, whichever
/// connection wrote: the library object, the rows its connection changed, the writes other connections
/// committed and the schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryRevision {
    instance: u64,
    total_changes: i64,
    data_version: i64,
    schema_version: i64,
}

impl LocalLibrary {
    pub fn revision(&self) -> Result<LibraryRevision> {
        let value = |sql: &str| self.conn.query_row(sql, [], |row| row.get::<_, i64>(0));
        Ok(LibraryRevision {
            instance: self.instance,
            total_changes: value("SELECT total_changes()")?,
            data_version: value("PRAGMA data_version")?,
            schema_version: value("PRAGMA schema_version")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::LocalLibrary;
    use crate::tests::make_track;

    #[test]
    fn revision_moves_with_every_write_and_not_with_reads() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let empty = lib.revision().unwrap();
        assert_eq!(lib.revision().unwrap(), empty);
        lib.get_all_playlists().unwrap();
        assert_eq!(lib.revision().unwrap(), empty);
        lib.add_track(&make_track("One", "/music/one.mp3")).unwrap();
        let one = lib.revision().unwrap();
        assert_ne!(one, empty);
        lib.conn.execute("UPDATE tracks SET title = 'Two'", []).unwrap();
        assert_ne!(lib.revision().unwrap(), one);
    }

    #[test]
    fn revision_differs_between_library_objects_and_sees_other_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        let (first, second) = (LocalLibrary::open(&path).unwrap(), LocalLibrary::open(&path).unwrap());
        assert_ne!(first.revision().unwrap(), second.revision().unwrap());
        let before = first.revision().unwrap();
        second.add_track(&make_track("One", "/music/one.mp3")).unwrap();
        assert_ne!(first.revision().unwrap(), before);
    }
}
