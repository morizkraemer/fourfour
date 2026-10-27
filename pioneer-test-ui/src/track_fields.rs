use pioneer_usb_writer::models::Track;
use serde::{Deserialize, Serialize};

const MAX_FIELD_CHARS: usize = 255;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackFieldEdits {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct TrackFields {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
}

impl TrackFields {
    pub fn of(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            genre: track.genre.clone(),
        }
    }
}

fn clean(name: &str, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.chars().count() > MAX_FIELD_CHARS {
        return Err(format!("{name} is longer than {MAX_FIELD_CHARS} characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{name} contains a control character"));
    }
    Ok(value.to_string())
}

pub fn apply_edits(track: &mut Track, edits: &TrackFieldEdits) -> Result<(), String> {
    let title = edits.title.as_deref().map(|v| clean("Title", v)).transpose()?;
    if title.as_deref() == Some("") {
        return Err("Title cannot be empty".to_string());
    }
    let artist = edits.artist.as_deref().map(|v| clean("Artist", v)).transpose()?;
    let album = edits.album.as_deref().map(|v| clean("Album", v)).transpose()?;
    let genre = edits.genre.as_deref().map(|v| clean("Genre", v)).transpose()?;
    for (target, value) in [
        (&mut track.title, title),
        (&mut track.artist, artist),
        (&mut track.album, album),
        (&mut track.genre, genre),
    ] {
        if let Some(value) = value {
            *target = value;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_library::LocalLibrary;
    use std::path::PathBuf;

    fn track() -> Track {
        Track {
            source_path: PathBuf::from("/music/a.mp3"),
            usb_path: "/Contents/a.mp3".into(),
            title: "File Title".into(),
            artist: "File Artist".into(),
            album: "File Album".into(),
            genre: "File Genre".into(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 2020,
            disc_number: 1,
            track_number: 1,
            tempo: 0,
            key: String::new(),
            duration_secs: 100.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1,
            id: 0,
            artwork: None,
        }
    }

    fn edits(title: Option<&str>, genre: Option<&str>) -> TrackFieldEdits {
        TrackFieldEdits { title: title.map(Into::into), genre: genre.map(Into::into), ..Default::default() }
    }

    #[test]
    fn edits_are_trimmed_and_stored_in_the_library_row() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&track()).unwrap();
        let mut stored = lib.get_track(id).unwrap().unwrap();
        apply_edits(&mut stored, &edits(Some("  New Title "), Some("Deep House"))).unwrap();
        lib.update_track(id, &stored).unwrap();

        let fields = TrackFields::of(&lib.get_track(id).unwrap().unwrap());
        assert_eq!(
            fields,
            TrackFields {
                title: "New Title".into(),
                artist: "File Artist".into(),
                album: "File Album".into(),
                genre: "Deep House".into()
            }
        );
    }

    #[test]
    fn clearing_an_optional_field_is_allowed() {
        let mut t = track();
        apply_edits(&mut t, &TrackFieldEdits { album: Some("  ".into()), ..Default::default() }).unwrap();
        assert_eq!(t.album, "");
    }

    #[test]
    fn bad_input_is_rejected_and_changes_nothing() {
        for bad in [edits(Some("   "), None), edits(Some(&"x".repeat(256)), None), edits(Some("Good"), Some("a\tb"))] {
            let mut t = track();
            let before = TrackFields::of(&t);
            assert!(apply_edits(&mut t, &bad).is_err());
            assert_eq!(TrackFields::of(&t), before);
        }
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(serde_json::from_str::<TrackFieldEdits>(r#"{"bpm":"1"}"#).is_err());
    }
}
