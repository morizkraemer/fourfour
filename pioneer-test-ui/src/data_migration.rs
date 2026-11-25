use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::app_log;

pub const LEGACY_IDENTIFIER: &str = "com.pioneer.test-ui";
const MARKER_NAME: &str = ".fourfour-migrated-from";
const STAGING_SUFFIX: &str = ".migrating";
const COMPARE_CHUNK: usize = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    NothingToMigrate,
    AlreadyMigrated,
    NewDirHasData,
    Renamed,
    Copied,
}

type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

pub fn app_dirs(os: Os, identifier: &str, env: Env) -> Vec<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.has_root());
    let under = |bases: Vec<Option<PathBuf>>| bases.into_iter().flatten().map(|base| base.join(identifier)).collect();
    match os {
        Os::Macos => {
            let library = absolute("HOME").map(|home| home.join("Library"));
            under(["Application Support", "Caches", "Logs", "WebKit"].iter().map(|dir| library.as_ref().map(|l| l.join(dir))).collect())
        }
        Os::Windows => under(vec![absolute("APPDATA"), absolute("LOCALAPPDATA")]),
        Os::Linux => {
            let home = absolute("HOME");
            let xdg = |name: &str, fallback: &str| absolute(name).or_else(|| home.as_ref().map(|h| h.join(fallback)));
            under(vec![
                xdg("XDG_DATA_HOME", ".local/share"),
                xdg("XDG_CONFIG_HOME", ".config"),
                xdg("XDG_CACHE_HOME", ".cache"),
            ])
        }
    }
}

pub fn migrate_legacy_data(identifier: &str, os: Os, env: Env) -> Vec<(PathBuf, Result<Outcome, String>)> {
    if identifier == LEGACY_IDENTIFIER {
        return Vec::new();
    }
    let legacy_dirs = app_dirs(os, LEGACY_IDENTIFIER, env);
    let new_dirs = app_dirs(os, identifier, env);
    legacy_dirs
        .into_iter()
        .zip(new_dirs)
        .map(|(old, new)| {
            let result = migrate_dir(&old, &new, |from, to| fs::rename(from, to)).map_err(|e| format!("{e:#}"));
            (old, result)
        })
        .collect()
}

pub fn log_migration(results: &[(PathBuf, Result<Outcome, String>)]) {
    for (old, result) in results {
        match result {
            Ok(Outcome::Renamed) => app_log::info(&format!("moved the data of {LEGACY_IDENTIFIER} from {}", old.display())),
            Ok(Outcome::Copied) => app_log::info(&format!(
                "copied the data of {LEGACY_IDENTIFIER} from {} (the old folder is kept)",
                old.display()
            )),
            Ok(Outcome::NewDirHasData) => app_log::info(&format!(
                "left {} alone: the new data folder already has data",
                old.display()
            )),
            Ok(Outcome::NothingToMigrate | Outcome::AlreadyMigrated) => {}
            Err(message) => app_log::error(&format!(
                "could not move the data of {LEGACY_IDENTIFIER} from {} (it is untouched, the app starts without it): {message}",
                old.display()
            )),
        }
    }
}

pub fn migrate_dir(
    old: &Path,
    new: &Path,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
) -> Result<Outcome, io::Error> {
    if new.join(MARKER_NAME).exists() {
        return Ok(Outcome::AlreadyMigrated);
    }
    if !is_non_empty_dir(old)? {
        return Ok(Outcome::NothingToMigrate);
    }
    if is_non_empty_dir(new)? {
        return Ok(Outcome::NewDirHasData);
    }
    match rename_old_over_new(old, new, &rename) {
        Ok(()) => Ok(Outcome::Renamed),
        Err(_) => copy_old_to_new(old, new).map(|()| Outcome::Copied),
    }
}

fn is_non_empty_dir(dir: &Path) -> io::Result<bool> {
    match fs::read_dir(dir) {
        Ok(mut entries) => Ok(entries.next().transpose()?.is_some()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io::Error::new(e.kind(), format!("{}: {e}", dir.display()))),
    }
}

fn write_marker(dir: &Path, old: &Path) -> io::Result<()> {
    fs::write(dir.join(MARKER_NAME), old.to_string_lossy().as_bytes())
}

fn rename_old_over_new(old: &Path, new: &Path, rename: &impl Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    write_marker(old, old)?;
    let moved = remove_empty_dir(new).and_then(|()| {
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent)?;
        }
        rename(old, new)
    });
    if moved.is_err() {
        let _ = fs::remove_file(old.join(MARKER_NAME));
    }
    moved
}

fn remove_empty_dir(dir: &Path) -> io::Result<()> {
    match fs::remove_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn copy_old_to_new(old: &Path, new: &Path) -> io::Result<()> {
    let staging = staging_dir(new);
    let _ = fs::remove_dir_all(&staging);
    let result = (|| {
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_tree(old, &staging)?;
        verify_tree(old, &staging)?;
        write_marker(&staging, old)?;
        remove_empty_dir(new)?;
        fs::rename(&staging, new)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result.map_err(|e| io::Error::new(e.kind(), format!("copy of {} to {} failed: {e}", old.display(), new.display())))
}

fn staging_dir(new: &Path) -> PathBuf {
    let mut name = new.file_name().map(OsString::from).unwrap_or_default();
    name.push(STAGING_SUFFIX);
    new.with_file_name(name)
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_file() {
            fs::copy(&source, &target)?;
        } else if kind.is_symlink() {
            copy_symlink(&source, &target)?;
        } else {
            return Err(io::Error::other(format!("unsupported entry {}", source.display())));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(source: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(source)?, target)
}

#[cfg(not(unix))]
fn copy_symlink(source: &Path, _target: &Path) -> io::Result<()> {
    Err(io::Error::other(format!("unsupported symlink {}", source.display())))
}

fn verify_tree(expected: &Path, actual: &Path) -> io::Result<()> {
    let names = |dir: &Path| -> io::Result<Vec<OsString>> {
        let mut names = fs::read_dir(dir)?.map(|e| e.map(|e| e.file_name())).collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    };
    if names(expected)? != names(actual)? {
        return Err(io::Error::other(format!("entries differ in {}", actual.display())));
    }
    for name in names(expected)? {
        let (source, copy) = (expected.join(&name), actual.join(&name));
        let (source_meta, copy_meta) = (fs::symlink_metadata(&source)?, fs::symlink_metadata(&copy)?);
        if source_meta.is_dir() && copy_meta.is_dir() {
            verify_tree(&source, &copy)?;
        } else if source_meta.is_file() && copy_meta.is_file() {
            if source_meta.len() != copy_meta.len() || !same_bytes(&source, &copy)? {
                return Err(io::Error::other(format!("{} differs from its copy", source.display())));
            }
        } else if source_meta.is_symlink() && copy_meta.is_symlink() {
            if fs::read_link(&source)? != fs::read_link(&copy)? {
                return Err(io::Error::other(format!("link {} differs from its copy", source.display())));
            }
        } else {
            return Err(io::Error::other(format!("{} has a different type in the copy", source.display())));
        }
    }
    Ok(())
}

fn same_bytes(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut a, mut b) = (File::open(a)?, File::open(b)?);
    let (mut buf_a, mut buf_b) = (vec![0u8; COMPARE_CHUNK], vec![0u8; COMPARE_CHUNK]);
    loop {
        let read_a = read_full(&mut a, &mut buf_a)?;
        let read_b = read_full(&mut b, &mut buf_b)?;
        if read_a != read_b || buf_a[..read_a] != buf_b[..read_b] {
            return Ok(false);
        }
        if read_a == 0 {
            return Ok(true);
        }
    }
}

fn read_full(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = std::env::var_os("CARGO_TARGET_TMPDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"));
            let dir = root.join(format!("data-migration-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn old_dir(&self) -> PathBuf {
            self.0.join("old")
        }

        fn new_dir(&self) -> PathBuf {
            self.0.join("sub").join("new")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn populate(dir: &Path) {
        fs::create_dir_all(dir.join("waveform-detail")).unwrap();
        fs::write(dir.join("library.db"), vec![7u8; COMPARE_CHUNK * 2 + 5]).unwrap();
        fs::write(dir.join("config.json"), b"{\"auto_analyze_new_tracks\": false}").unwrap();
        fs::write(dir.join("waveform-detail").join("a.bin"), b"detail").unwrap();
    }

    fn rename_fails(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::other("cross-device link"))
    }

    fn plain_rename(from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn assert_populated(dir: &Path) {
        assert_eq!(fs::read(dir.join("library.db")).unwrap().len(), COMPARE_CHUNK * 2 + 5);
        assert_eq!(fs::read(dir.join("waveform-detail").join("a.bin")).unwrap(), b"detail");
        assert_eq!(fs::read(dir.join("config.json")).unwrap(), b"{\"auto_analyze_new_tracks\": false}");
    }

    #[test]
    fn a_missing_new_dir_gets_the_old_data_by_rename() {
        let scratch = Scratch::new("rename");
        populate(&scratch.old_dir());
        let outcome = migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap();
        assert_eq!(outcome, Outcome::Renamed);
        assert_populated(&scratch.new_dir());
        assert!(scratch.new_dir().join(MARKER_NAME).exists());
        assert!(!scratch.old_dir().exists());
    }

    #[test]
    fn an_empty_new_dir_counts_as_missing() {
        let scratch = Scratch::new("empty-new");
        populate(&scratch.old_dir());
        fs::create_dir_all(scratch.new_dir()).unwrap();
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap(), Outcome::Renamed);
        assert_populated(&scratch.new_dir());
    }

    #[test]
    fn when_rename_is_not_possible_the_data_is_copied_and_the_old_dir_is_kept() {
        let scratch = Scratch::new("copy");
        populate(&scratch.old_dir());
        let outcome = migrate_dir(&scratch.old_dir(), &scratch.new_dir(), rename_fails).unwrap();
        assert_eq!(outcome, Outcome::Copied);
        assert_populated(&scratch.new_dir());
        assert_populated(&scratch.old_dir());
        assert!(scratch.new_dir().join(MARKER_NAME).exists());
        assert!(!scratch.old_dir().join(MARKER_NAME).exists());
        assert!(!staging_dir(&scratch.new_dir()).exists());
    }

    #[test]
    fn an_old_dir_that_does_not_exist_or_is_empty_has_nothing_to_migrate() {
        let scratch = Scratch::new("nothing");
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap(), Outcome::NothingToMigrate);
        fs::create_dir_all(scratch.old_dir()).unwrap();
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap(), Outcome::NothingToMigrate);
        assert!(!scratch.new_dir().exists());
    }

    #[test]
    fn a_new_dir_that_already_has_data_is_left_alone_and_the_old_one_too() {
        let scratch = Scratch::new("both");
        populate(&scratch.old_dir());
        fs::create_dir_all(scratch.new_dir()).unwrap();
        fs::write(scratch.new_dir().join("library.db"), b"fresh").unwrap();
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap(), Outcome::NewDirHasData);
        assert_eq!(fs::read(scratch.new_dir().join("library.db")).unwrap(), b"fresh");
        assert_populated(&scratch.old_dir());
        assert!(!scratch.new_dir().join(MARKER_NAME).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_copy_leaves_the_old_data_untouched_and_no_new_dir() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let scratch = Scratch::new("partial");
        populate(&scratch.old_dir());
        let fifo = CString::new(scratch.old_dir().join("pipe").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let error = migrate_dir(&scratch.old_dir(), &scratch.new_dir(), rename_fails).unwrap_err();
        assert!(error.to_string().contains("unsupported entry"), "{error}");
        assert_populated(&scratch.old_dir());
        assert!(!scratch.new_dir().exists());
        assert!(!staging_dir(&scratch.new_dir()).exists());
        assert!(!scratch.old_dir().join(MARKER_NAME).exists());
    }

    #[test]
    fn a_failed_rename_takes_its_marker_back_out_of_the_old_dir() {
        let scratch = Scratch::new("rename-marker");
        populate(&scratch.old_dir());
        assert!(rename_old_over_new(&scratch.old_dir(), &scratch.new_dir(), &rename_fails).is_err());
        assert!(!scratch.old_dir().join(MARKER_NAME).exists());
    }

    #[test]
    fn running_twice_changes_nothing_the_second_time() {
        let scratch = Scratch::new("twice");
        populate(&scratch.old_dir());
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), rename_fails).unwrap(), Outcome::Copied);
        fs::write(scratch.new_dir().join("library.db"), b"edited after the move").unwrap();
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), rename_fails).unwrap(), Outcome::AlreadyMigrated);
        assert_eq!(fs::read(scratch.new_dir().join("library.db")).unwrap(), b"edited after the move");
        fs::remove_dir_all(scratch.old_dir()).unwrap();
        populate(&scratch.old_dir());
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), plain_rename).unwrap(), Outcome::AlreadyMigrated);
    }

    #[test]
    fn a_leftover_staging_dir_from_a_crashed_copy_is_replaced() {
        let scratch = Scratch::new("stale-staging");
        populate(&scratch.old_dir());
        fs::create_dir_all(staging_dir(&scratch.new_dir())).unwrap();
        fs::write(staging_dir(&scratch.new_dir()).join("junk"), b"x").unwrap();
        assert_eq!(migrate_dir(&scratch.old_dir(), &scratch.new_dir(), rename_fails).unwrap(), Outcome::Copied);
        assert!(!scratch.new_dir().join("junk").exists());
        assert!(!staging_dir(&scratch.new_dir()).exists());
    }

    #[test]
    fn verification_catches_a_copy_that_differs() {
        let scratch = Scratch::new("verify");
        populate(&scratch.old_dir());
        fs::create_dir_all(scratch.new_dir().parent().unwrap()).unwrap();
        copy_tree(&scratch.old_dir(), &scratch.new_dir()).unwrap();
        verify_tree(&scratch.old_dir(), &scratch.new_dir()).unwrap();
        let mut bytes = fs::read(scratch.new_dir().join("library.db")).unwrap();
        *bytes.last_mut().unwrap() = 8;
        fs::write(scratch.new_dir().join("library.db"), bytes).unwrap();
        assert!(verify_tree(&scratch.old_dir(), &scratch.new_dir()).is_err());
    }

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| OsString::from(*v))
    }

    #[test]
    fn the_app_dirs_follow_the_platform_layout() {
        let mac = app_dirs(Os::Macos, "app.x", &env_of(&[("HOME", "/Users/dj")]));
        let library = PathBuf::from("/Users/dj").join("Library");
        assert_eq!(
            mac,
            [
                library.join("Application Support").join("app.x"),
                library.join("Caches").join("app.x"),
                library.join("Logs").join("app.x"),
                library.join("WebKit").join("app.x"),
            ]
        );
        let windows = app_dirs(Os::Windows, "app.x", &env_of(&[("APPDATA", "/r"), ("LOCALAPPDATA", "/l")]));
        assert_eq!(windows, [PathBuf::from("/r/app.x"), PathBuf::from("/l/app.x")]);
        let linux = app_dirs(Os::Linux, "app.x", &env_of(&[("HOME", "/home/dj"), ("XDG_DATA_HOME", "/xdg")]));
        assert_eq!(
            linux,
            [PathBuf::from("/xdg/app.x"), PathBuf::from("/home/dj/.config/app.x"), PathBuf::from("/home/dj/.cache/app.x")]
        );
        assert!(app_dirs(Os::Linux, "app.x", &env_of(&[])).is_empty());
    }

    #[test]
    fn migrating_for_an_identifier_moves_each_legacy_dir_and_skips_the_legacy_id_itself() {
        let scratch = Scratch::new("all");
        let home = scratch.0.join("home");
        let home_text: &'static str = Box::leak(home.to_string_lossy().into_owned().into_boxed_str());
        let pairs: &'static [(&'static str, &'static str)] = Box::leak(vec![("HOME", home_text)].into_boxed_slice());
        let env = env_of(pairs);
        let legacy = app_dirs(Os::Macos, LEGACY_IDENTIFIER, &env);
        populate(&legacy[0]);
        fs::create_dir_all(&legacy[2]).unwrap();
        fs::write(legacy[2].join("fourfour.log"), b"old log").unwrap();
        assert!(migrate_legacy_data(LEGACY_IDENTIFIER, Os::Macos, &env).is_empty());
        let results = migrate_legacy_data("app.x", Os::Macos, &env);
        let outcomes: Vec<_> = results.into_iter().map(|(_, r)| r.unwrap()).collect();
        assert_eq!(outcomes, [Outcome::Renamed, Outcome::NothingToMigrate, Outcome::Renamed, Outcome::NothingToMigrate]);
        let new = app_dirs(Os::Macos, "app.x", &env);
        assert_populated(&new[0]);
        assert_eq!(fs::read(new[2].join("fourfour.log")).unwrap(), b"old log");
    }
}
