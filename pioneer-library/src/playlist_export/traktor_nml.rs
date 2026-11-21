use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::xml;
use super::{ExportDocument, ExportNode, ExportTrack};

const FALLBACK_VOLUME: &str = "Macintosh HD";
const CUE_TYPE_CUE: &str = "0";
const CUE_TYPE_GRID: &str = "4";
const CUE_TYPE_LOOP: &str = "5";

/// Traktor's musical key number (0 to 11 major from C, 12 to 23 minor from C) and Open Key name for a Camelot key.
pub fn traktor_key(camelot: &str) -> Option<(u8, String)> {
    let letter = camelot.chars().last()?;
    let number: i32 = camelot[..camelot.len() - letter.len_utf8()].parse().ok().filter(|n| (1..=12).contains(n))?;
    let major_root = ((number - 8).rem_euclid(12) * 7).rem_euclid(12) as u8;
    let open_number = (number - 8).rem_euclid(12) + 1;
    match letter.to_ascii_uppercase() {
        'B' => Some((major_root, format!("{open_number}d"))),
        'A' => Some((12 + (major_root + 9) % 12, format!("{open_number}m"))),
        _ => None,
    }
}

pub struct NmlLocation {
    pub volume: String,
    pub dir: String,
    pub file: String,
}

impl NmlLocation {
    pub fn of(path: &str) -> Self {
        let slashed = path.replace('\\', "/");
        let mut parts: Vec<&str> = slashed.split('/').filter(|p| !p.is_empty()).collect();
        let file = parts.pop().unwrap_or_default().to_string();
        let drive = slashed.as_bytes().get(1) == Some(&b':');
        let volume = if drive {
            parts.remove(0).to_string()
        } else if parts.first() == Some(&"Volumes") && parts.len() > 1 {
            parts.remove(0);
            parts.remove(0).to_string()
        } else {
            FALLBACK_VOLUME.to_string()
        };
        let dir = if parts.is_empty() { "/:".to_string() } else { format!("/:{}/:", parts.join("/:")) };
        Self { volume, dir, file }
    }

    pub fn key(&self) -> String {
        format!("{}{}{}", self.volume, self.dir, self.file)
    }
}

fn uuid_of(name: &str) -> String {
    let mut hash = DefaultHasher::new();
    name.hash(&mut hash);
    let high = hash.finish();
    "fourfour".hash(&mut hash);
    format!("{high:016x}{:016x}", hash.finish())
}

fn cue_attrs(name: &str, kind: &str, start_ms: u32, len_ms: u32, hotcue: i64) -> Vec<(&'static str, String)> {
    vec![
        ("NAME", if name.is_empty() { "n.a.".to_string() } else { name.to_string() }),
        ("DISPL_ORDER", "0".to_string()),
        ("TYPE", kind.to_string()),
        ("START", format!("{start_ms}.000000")),
        ("LEN", format!("{len_ms}.000000")),
        ("REPEATS", "-1".to_string()),
        ("HOTCUE", hotcue.to_string()),
    ]
}

fn write_entry(out: &mut String, track: &ExportTrack) {
    let t = &track.track;
    let location = NmlLocation::of(&track.path);
    xml::start(out, 2, "ENTRY", &[("TITLE", t.title.clone()), ("ARTIST", t.artist.clone())], false);
    let location_attrs = [("DIR", location.dir.clone()), ("FILE", location.file.clone()), ("VOLUME", location.volume.clone())];
    xml::start(out, 3, "LOCATION", &location_attrs, true);
    xml::start(out, 3, "ALBUM", &[("TRACK", t.track_number.to_string()), ("TITLE", t.album.clone())], true);
    let key = traktor_key(&track.key);
    let mut info = vec![
        ("BITRATE", (t.bitrate as u64 * 1000).to_string()),
        ("GENRE", t.genre.clone()),
        ("LABEL", t.label.clone()),
        ("COMMENT", t.comment.clone()),
    ];
    if !t.remixer.is_empty() {
        info.push(("REMIXER", t.remixer.clone()));
    }
    if let Some((_, open)) = &key {
        info.push(("KEY", open.clone()));
    }
    info.extend([
        ("PLAYTIME", (t.duration_secs.round() as u64).to_string()),
        ("PLAYTIME_FLOAT", format!("{:.6}", t.duration_secs)),
        ("RANKING", (track.rating.unwrap_or(0) as u32 * 51).to_string()),
        ("FILESIZE", (t.file_size / 1024).to_string()),
    ]);
    if t.year > 0 {
        info.push(("RELEASE_DATE", format!("{}/1/1", t.year)));
    }
    xml::start(out, 3, "INFO", &info, true);
    xml::start(out, 3, "TEMPO", &[("BPM", format!("{:.6}", track.bpm)), ("BPM_QUALITY", "100".into())], true);
    if let Some((value, _)) = key {
        xml::start(out, 3, "MUSICAL_KEY", &[("VALUE", value.to_string())], true);
    }
    let downbeat = track.grid.iter().find(|b| b.bar_position == 1).or(track.grid.first());
    if let Some(beat) = downbeat {
        xml::start(out, 3, "CUE_V2", &cue_attrs("AutoGrid", CUE_TYPE_GRID, beat.time_ms, 0, 0), true);
    }
    for cue in &track.cues {
        let hotcue = cue.hot_cue_number as i64 - 1;
        let attrs = match cue.loop_time_ms {
            Some(end) => cue_attrs(&cue.name, CUE_TYPE_LOOP, cue.time_ms, end.saturating_sub(cue.time_ms), hotcue),
            None => cue_attrs(&cue.name, CUE_TYPE_CUE, cue.time_ms, 0, hotcue),
        };
        xml::start(out, 3, "CUE_V2", &attrs, true);
    }
    xml::end(out, 2, "ENTRY");
}

fn write_node(out: &mut String, depth: usize, node: &ExportNode, doc: &ExportDocument) {
    match node {
        ExportNode::Folder { name, children } => {
            xml::start(out, depth, "NODE", &[("TYPE", "FOLDER".into()), ("NAME", name.clone())], false);
            xml::start(out, depth + 1, "SUBNODES", &[("COUNT", children.len().to_string())], false);
            for child in children {
                write_node(out, depth + 2, child, doc);
            }
            xml::end(out, depth + 1, "SUBNODES");
            xml::end(out, depth, "NODE");
        }
        ExportNode::Playlist { name, track_ids } => {
            xml::start(out, depth, "NODE", &[("TYPE", "PLAYLIST".into()), ("NAME", name.clone())], false);
            let attrs = [("ENTRIES", track_ids.len().to_string()), ("TYPE", "LIST".into()), ("UUID", uuid_of(name))];
            xml::start(out, depth + 1, "PLAYLIST", &attrs, false);
            for track in track_ids.iter().filter_map(|id| doc.track(*id)) {
                xml::start(out, depth + 2, "ENTRY", &[], false);
                let key = NmlLocation::of(&track.path).key();
                xml::start(out, depth + 3, "PRIMARYKEY", &[("TYPE", "TRACK".into()), ("KEY", key)], true);
                xml::end(out, depth + 2, "ENTRY");
            }
            xml::end(out, depth + 1, "PLAYLIST");
            xml::end(out, depth, "NODE");
        }
    }
}

pub fn render(doc: &ExportDocument) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    xml::start(&mut out, 0, "NML", &[("VERSION", "19".into())], false);
    xml::start(&mut out, 1, "HEAD", &[("COMPANY", "www.native-instruments.com".into()), ("PROGRAM", "Traktor".into())], true);
    xml::start(&mut out, 1, "COLLECTION", &[("ENTRIES", doc.tracks.len().to_string())], false);
    for track in &doc.tracks {
        write_entry(&mut out, track);
    }
    xml::end(&mut out, 1, "COLLECTION");
    xml::start(&mut out, 1, "PLAYLISTS", &[], false);
    xml::start(&mut out, 2, "NODE", &[("TYPE", "FOLDER".into()), ("NAME", "$ROOT".into())], false);
    xml::start(&mut out, 3, "SUBNODES", &[("COUNT", "1".into())], false);
    write_node(&mut out, 4, &doc.root, doc);
    xml::end(&mut out, 3, "SUBNODES");
    xml::end(&mut out, 2, "NODE");
    xml::end(&mut out, 1, "PLAYLISTS");
    xml::end(&mut out, 0, "NML");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camelot_keys_map_to_traktor_numbers_and_open_key() {
        assert_eq!(traktor_key("8B"), Some((0, "1d".into())));
        assert_eq!(traktor_key("8A"), Some((21, "1m".into())));
        assert_eq!(traktor_key("1B"), Some((11, "6d".into())));
        assert_eq!(traktor_key("12A"), Some((13, "5m".into())));
        assert_eq!(traktor_key("13A"), None);
        assert_eq!(traktor_key(""), None);
    }

    #[test]
    fn locations_split_into_volume_dir_and_file() {
        let mac = NmlLocation::of("/Users/me/Music/a.mp3");
        assert_eq!((mac.volume.as_str(), mac.dir.as_str(), mac.file.as_str()), ("Macintosh HD", "/:Users/:me/:Music/:", "a.mp3"));
        assert_eq!(mac.key(), "Macintosh HD/:Users/:me/:Music/:a.mp3");
        let external = NmlLocation::of("/Volumes/Tunes/House/b.flac");
        assert_eq!((external.volume.as_str(), external.dir.as_str()), ("Tunes", "/:House/:"));
        let windows = NmlLocation::of("C:\\Music\\c.wav");
        assert_eq!((windows.volume.as_str(), windows.dir.as_str(), windows.file.as_str()), ("C:", "/:Music/:", "c.wav"));
    }
}
