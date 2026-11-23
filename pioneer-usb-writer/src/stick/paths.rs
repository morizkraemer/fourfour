use std::collections::BTreeSet;

pub const MAX_COMPONENT_UNITS: usize = 255;
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

fn truncate_units(text: &str, max: usize) -> &str {
    let mut used = 0;
    for (at, ch) in text.char_indices() {
        used += ch.len_utf16();
        if used > max {
            return &text[..at];
        }
    }
    text
}

fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 && at + 1 < name.len() => (&name[..at], &name[at..]),
        _ => (name, ""),
    }
}

pub fn safe_component(component: &str) -> String {
    let replaced: String = component
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();
    let trimmed = replaced.trim_end_matches(['.', ' ']).trim_start_matches(' ');
    let mut name = if trimmed.is_empty() { "_".to_owned() } else { trimmed.to_owned() };
    let stem = split_extension(&name).0;
    if RESERVED_NAMES.iter().any(|reserved| reserved.eq_ignore_ascii_case(stem)) {
        name.insert(0, '_');
    }
    if units(&name) > MAX_COMPONENT_UNITS {
        let (stem, extension) = split_extension(&name);
        let extension = if units(extension) < 16 { extension } else { "" };
        let kept = truncate_units(stem, MAX_COMPONENT_UNITS - units(extension));
        name = format!("{}{extension}", kept.trim_end_matches(['.', ' ']));
    }
    name
}

pub fn safe_stick_path(path: &str) -> String {
    let components: Vec<String> =
        path.split('/').filter(|component| !component.is_empty()).map(safe_component).collect();
    format!("/{}", components.join("/"))
}

pub fn fold(path: &str) -> String {
    path.to_lowercase()
}

fn numbered(path: &str, number: usize) -> String {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    let (stem, extension) = split_extension(file);
    let suffix = format!(" ({number})");
    let kept = truncate_units(stem, MAX_COMPONENT_UNITS.saturating_sub(units(extension) + units(&suffix)));
    format!("{dir}/{kept}{suffix}{extension}")
}

pub fn unique_stick_path(wanted: &str, taken: &mut BTreeSet<String>) -> String {
    let safe = safe_stick_path(wanted);
    let mut candidate = safe.clone();
    let mut number = 2;
    while !taken.insert(fold(&candidate)) {
        candidate = numbered(&safe, number);
        number += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_dots_and_spaces_leave_folder_and_file_names() {
        assert_eq!(safe_stick_path("/Contents/Artist Name./Track .wav"), "/Contents/Artist Name/Track .wav");
        assert_eq!(safe_stick_path("/Contents/Dots.../a.mp3"), "/Contents/Dots/a.mp3");
        assert_eq!(safe_stick_path("/Contents/ . /a.mp3"), "/Contents/_/a.mp3");
        assert_eq!(safe_component("name. . "), "name");
    }

    #[test]
    fn illegal_characters_and_reserved_names_are_replaced() {
        assert_eq!(safe_component("a?b:c*d"), "a_b_c_d");
        assert_eq!(safe_component("con"), "_con");
        assert_eq!(safe_component("COM1.wav"), "_COM1.wav");
        assert_eq!(safe_component("Console"), "Console");
        assert_eq!(safe_component("tab\there"), "tab_here");
    }

    #[test]
    fn long_names_are_cut_to_255_units_and_keep_the_extension() {
        let long = format!("{}.flac", "\u{65e5}".repeat(300));
        let safe = safe_component(&long);
        assert_eq!(safe.encode_utf16().count(), 255);
        assert!(safe.ends_with(".flac"));
        assert_eq!(safe_component("short.wav"), "short.wav");
    }

    #[test]
    fn paths_that_differ_only_in_case_get_a_number() {
        let mut taken = BTreeSet::new();
        assert_eq!(unique_stick_path("/Contents/A/Track.wav", &mut taken), "/Contents/A/Track.wav");
        assert_eq!(unique_stick_path("/Contents/a/track.WAV", &mut taken), "/Contents/a/track (2).WAV");
        assert_eq!(unique_stick_path("/Contents/A/TRACK.wav", &mut taken), "/Contents/A/TRACK (3).wav");
        assert_eq!(unique_stick_path("/Contents/A/Other.wav", &mut taken), "/Contents/A/Other.wav");
        assert_eq!(unique_stick_path("/Contents/A./Other.wav", &mut taken), "/Contents/A/Other (2).wav");
    }
}
