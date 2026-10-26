use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

fn leading_separator(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\')
}

/// Prefix replacement applied to paths stored by another machine. Matching is
/// case-insensitive and treats `/` and `\` alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRemap {
    pub from_prefix: String,
    pub to_prefix: String,
}

impl PathRemap {
    pub fn apply(&self, path: &str) -> PathBuf {
        if !self.matches(path) {
            return PathBuf::from(path);
        }
        let skip = components(&self.from_prefix).len();
        let mut out = PathBuf::from(&self.to_prefix);
        for part in components(path).into_iter().skip(skip) {
            out.push(part);
        }
        out
    }

    pub fn matches(&self, path: &str) -> bool {
        let from = components(&self.from_prefix);
        let parts = components(path);
        parts.len() >= from.len()
            && from
                .iter()
                .zip(&parts)
                .all(|(a, b)| a.to_lowercase() == b.to_lowercase())
    }
}

/// Longest directory prefix shared by all paths, compared case-insensitively on
/// whole path components. Returns the spelling used by the first path.
pub fn detect_common_root(paths: &[&str]) -> String {
    let Some(first) = paths.first() else {
        return String::new();
    };
    let first_parts = components(first);
    let dir_len = first_parts.len().saturating_sub(1);
    let mut common = dir_len;
    for path in &paths[1..] {
        let parts = components(path);
        let limit = common.min(parts.len().saturating_sub(1));
        let mut shared = 0;
        while shared < limit && first_parts[shared].to_lowercase() == parts[shared].to_lowercase() {
            shared += 1;
        }
        common = shared;
        if common == 0 {
            break;
        }
    }
    let joined = first_parts[..common].join("/");
    if leading_separator(first) {
        format!("/{joined}")
    } else {
        joined
    }
}

/// Resolves paths against the real filesystem ignoring letter case, caching
/// directory listings so thousands of lookups stay cheap.
#[derive(Default)]
pub struct CaseInsensitiveResolver {
    listings: HashMap<PathBuf, HashMap<String, String>>,
}

impl CaseInsensitiveResolver {
    pub fn resolve(&mut self, path: &Path) -> Option<PathBuf> {
        if path.exists() {
            return Some(path.to_path_buf());
        }
        let mut current = PathBuf::new();
        for component in path.components() {
            use std::path::Component;
            match component {
                Component::Normal(name) => {
                    let name = name.to_string_lossy();
                    let next = current.join(name.as_ref());
                    if next.exists() {
                        current = next;
                        continue;
                    }
                    let listing = self.listing(&current);
                    let actual = listing.get(&name.to_lowercase())?;
                    current.push(actual);
                }
                other => current.push(other.as_os_str()),
            }
        }
        Some(current)
    }

    fn listing(&mut self, dir: &Path) -> &HashMap<String, String> {
        self.listings.entry(dir.to_path_buf()).or_insert_with(|| {
            let target = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
            std::fs::read_dir(target)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            (name.to_lowercase(), name)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_root_is_component_wise_and_case_insensitive() {
        let root = detect_common_root(&[
            "/Users/me/Music/House/a.mp3",
            "/users/me/music/Techno/b.mp3",
            "/Users/me/Music/c.mp3",
        ]);
        assert_eq!(root, "/Users/me/Music");
    }

    #[test]
    fn common_root_of_single_path_is_its_directory() {
        assert_eq!(detect_common_root(&["/a/b/c.mp3"]), "/a/b");
    }

    #[test]
    fn remap_swaps_prefix_case_insensitively() {
        let remap = PathRemap {
            from_prefix: "/Users/Me/Music/".into(),
            to_prefix: "/mnt/audio".into(),
        };
        assert_eq!(
            remap.apply("/users/me/music/House/a.mp3"),
            PathBuf::from("/mnt/audio/House/a.mp3")
        );
        assert_eq!(remap.apply("/other/a.mp3"), PathBuf::from("/other/a.mp3"));
    }

    #[test]
    fn remap_handles_windows_separators() {
        let remap = PathRemap {
            from_prefix: "C:\\Music".into(),
            to_prefix: "/mnt/c/Music".into(),
        };
        assert_eq!(
            remap.apply("c:/music/House\\a.mp3"),
            PathBuf::from("/mnt/c/Music/House/a.mp3")
        );
    }

    #[test]
    fn resolver_finds_files_with_different_case() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("House")).unwrap();
        std::fs::write(dir.path().join("House/Track One.mp3"), b"x").unwrap();
        let mut resolver = CaseInsensitiveResolver::default();
        let found = resolver.resolve(&dir.path().join("house/track one.MP3"));
        assert_eq!(found, Some(dir.path().join("House/Track One.mp3")));
        assert_eq!(resolver.resolve(&dir.path().join("house/missing.mp3")), None);
    }
}
