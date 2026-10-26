pub const CLASSES: usize = 24;

pub fn tonic_of(class: usize) -> usize {
    class % 12
}

pub fn is_minor(class: usize) -> bool {
    class >= 12
}

pub fn camelot(class: usize) -> &'static str {
    const MAJOR: [&str; 12] = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"];
    const MINOR: [&str; 12] = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"];
    if is_minor(class) { MINOR[tonic_of(class)] } else { MAJOR[tonic_of(class)] }
}

pub fn class_from_camelot(code: &str) -> Option<usize> {
    (0..CLASSES).find(|c| camelot(*c) == code)
}
