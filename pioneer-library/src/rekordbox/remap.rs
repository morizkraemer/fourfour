use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

/// Comparison form of a file name or path: NFC (macOS stores NFD) and lowercase.
pub fn fold(text: &str) -> String {
    text.nfc().collect::<String>().to_lowercase()
}

fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

fn leading_separator(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\')
}

/// Prefix replacement applied to paths stored by another machine. Matching is
/// case-insensitive, Unicode-normalisation-insensitive and treats `/` and `\` alike.
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
                .all(|(a, b)| fold(a) == fold(b))
    }
}

/// Ordered remap rules; the first rule whose prefix matches a path wins. Paths no
/// rule matches go through `fallback` (a source's implied remap), if any.
#[derive(Debug, Clone, Default)]
pub struct RemapPlan {
    pub rules: Vec<PathRemap>,
    pub fallback: Option<PathRemap>,
}

impl RemapPlan {
    pub fn rule_index(&self, path: &str) -> Option<usize> {
        self.rules.iter().position(|r| r.matches(path))
    }

    pub fn apply(&self, path: &str) -> PathBuf {
        match self.rule_index(path) {
            Some(i) => self.rules[i].apply(path),
            None => self.fallback.as_ref().map_or_else(|| PathBuf::from(path), |r| r.apply(path)),
        }
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
        while shared < limit && fold(first_parts[shared]) == fold(parts[shared]) {
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
                    let actual = listing.get(&fold(&name))?;
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
                            (fold(&name), name)
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
    fn plan_uses_first_matching_rule_then_fallback() {
        let plan = RemapPlan {
            rules: vec![
                PathRemap { from_prefix: "/Users/me/Music".into(), to_prefix: "/mnt/a".into() },
                PathRemap { from_prefix: "/Users/me/Dev".into(), to_prefix: "/mnt/b".into() },
            ],
            fallback: Some(PathRemap { from_prefix: "/Volumes/X".into(), to_prefix: "/mnt/x".into() }),
        };
        assert_eq!(plan.apply("/Users/me/Music/a.mp3"), PathBuf::from("/mnt/a/a.mp3"));
        assert_eq!(plan.apply("/users/me/dev/s/b.mp3"), PathBuf::from("/mnt/b/s/b.mp3"));
        assert_eq!(plan.apply("/Volumes/X/c.mp3"), PathBuf::from("/mnt/x/c.mp3"));
        assert_eq!(plan.apply("/else/d.mp3"), PathBuf::from("/else/d.mp3"));
        assert_eq!(plan.rule_index("/else/d.mp3"), None);
    }

    #[test]
    fn remap_maps_windows_drive_paths_with_either_separator() {
        let remap = PathRemap { from_prefix: "C:\\Music".into(), to_prefix: "/mnt/music".into() };
        assert_eq!(remap.apply("C:/Music/House/a.mp3"), PathBuf::from("/mnt/music/House/a.mp3"));
        assert_eq!(remap.apply("c:\\music\\House\\a.mp3"), PathBuf::from("/mnt/music/House/a.mp3"));
        assert!(!remap.matches("D:/Music/a.mp3"));
    }

    #[test]
    fn remap_matches_across_unicode_normalisation() {
        let remap = PathRemap { from_prefix: "/Users/me/Mu\u{308}sik".into(), to_prefix: "/mnt".into() };
        assert!(remap.matches("/Users/me/M\u{fc}sik/a.mp3"));
        assert_eq!(fold("Cafe\u{301}"), fold("Caf\u{e9}"));
    }

    #[test]
    fn resolver_finds_nfd_name_for_nfc_request_and_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Caf\u{e9} del Mar.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("Mu\u{308}sik.mp3"), b"x").unwrap();
        let mut resolver = CaseInsensitiveResolver::default();
        assert_eq!(
            resolver.resolve(&dir.path().join("Cafe\u{301} del Mar.mp3")),
            Some(dir.path().join("Caf\u{e9} del Mar.mp3"))
        );
        assert_eq!(
            resolver.resolve(&dir.path().join("M\u{fc}sik.mp3")),
            Some(dir.path().join("Mu\u{308}sik.mp3"))
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
