use super::xml::{self, extension, rgb, seconds};
use super::{ExportDocument, ExportNode, ExportTrack};
use pioneer_usb_writer::models::Beat;

const GRID_TOLERANCE_MS: f64 = 10.0;

#[derive(Debug, PartialEq)]
pub struct TempoMarker {
    pub start_ms: u32,
    pub bpm: f64,
    pub first_beat_in_bar: u8,
}

/// The beat grid as Rekordbox tempo markers: one marker as long as the beats keep their tempo, spacing and bar
/// count to within 10 ms, a new one where they stop doing that.
pub fn tempo_markers(beats: &[Beat], fallback_bpm: f64) -> Vec<TempoMarker> {
    let mut markers = Vec::new();
    let mut i = 0;
    while i < beats.len() {
        let first = beats[i];
        let bpm = if first.tempo > 0 { first.tempo as f64 / 100.0 } else { fallback_bpm };
        if bpm <= 0.0 {
            i += 1;
            continue;
        }
        let beat_ms = 60_000.0 / bpm;
        let mut next = i + 1;
        while let Some(beat) = beats.get(next) {
            let steps = (next - i) as f64;
            let on_grid = (beat.time_ms as f64 - (first.time_ms as f64 + steps * beat_ms)).abs() <= GRID_TOLERANCE_MS;
            let expected_bar = (first.bar_position as usize - 1 + (next - i)) % 4 + 1;
            if !on_grid || beat.bar_position as usize != expected_bar {
                break;
            }
            next += 1;
        }
        markers.push(TempoMarker { start_ms: first.time_ms, bpm, first_beat_in_bar: first.bar_position.clamp(1, 4) });
        i = next;
    }
    markers
}

pub fn location(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    let bytes = slashed.as_bytes();
    let drive = bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let mut out = String::from("file://localhost");
    if drive {
        out.push('/');
    }
    for &byte in slashed.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~/:".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn kind(path: &str) -> Option<&'static str> {
    Some(match extension(path).as_str() {
        "mp3" => "MP3 File",
        "wav" => "WAV File",
        "flac" => "FLAC File",
        "aif" | "aiff" => "AIFF File",
        "m4a" => "M4A File",
        "aac" => "AAC File",
        _ => return None,
    })
}

fn track_attrs(track: &ExportTrack) -> Vec<(&'static str, String)> {
    let t = &track.track;
    let mut attrs = vec![
        ("TrackID", track.id.to_string()),
        ("Name", t.title.clone()),
        ("Artist", t.artist.clone()),
        ("Album", t.album.clone()),
        ("Genre", t.genre.clone()),
    ];
    if let Some(kind) = kind(&track.path) {
        attrs.push(("Kind", kind.to_string()));
    }
    attrs.extend([
        ("Size", t.file_size.to_string()),
        ("TotalTime", (t.duration_secs.round() as u64).to_string()),
        ("DiscNumber", t.disc_number.to_string()),
        ("TrackNumber", t.track_number.to_string()),
        ("Year", t.year.to_string()),
        ("AverageBpm", format!("{:.2}", track.bpm)),
        ("BitRate", t.bitrate.to_string()),
        ("SampleRate", t.sample_rate.to_string()),
        ("Comments", t.comment.clone()),
        ("Rating", (track.rating.unwrap_or(0) as u32 * 51).to_string()),
        ("Location", location(&track.path)),
        ("Remixer", t.remixer.clone()),
        ("Tonality", track.key.clone()),
        ("Label", t.label.clone()),
    ]);
    if let Some([r, g, b]) = track.color.as_deref().and_then(rgb) {
        attrs.push(("Colour", format!("0x{r:02X}{g:02X}{b:02X}")));
    }
    attrs
}

fn write_track(out: &mut String, track: &ExportTrack) {
    let markers = tempo_markers(&track.grid, track.bpm);
    if markers.is_empty() && track.cues.is_empty() {
        xml::start(out, 2, "TRACK", &track_attrs(track), true);
        return;
    }
    xml::start(out, 2, "TRACK", &track_attrs(track), false);
    for marker in &markers {
        let attrs = [
            ("Inizio", seconds(marker.start_ms)),
            ("Bpm", format!("{:.2}", marker.bpm)),
            ("Metro", "4/4".to_string()),
            ("Battito", marker.first_beat_in_bar.to_string()),
        ];
        xml::start(out, 3, "TEMPO", &attrs, true);
    }
    for cue in &track.cues {
        let mut attrs = vec![
            ("Name", cue.name.clone()),
            ("Type", if cue.loop_time_ms.is_some() { "4" } else { "0" }.to_string()),
            ("Start", seconds(cue.time_ms)),
        ];
        if let Some(end) = cue.loop_time_ms {
            attrs.push(("End", seconds(end)));
        }
        attrs.push(("Num", (cue.hot_cue_number as i64 - 1).to_string()));
        if let Some([r, g, b]) = cue.color.as_deref().and_then(rgb) {
            attrs.extend([("Red", r.to_string()), ("Green", g.to_string()), ("Blue", b.to_string())]);
        }
        xml::start(out, 3, "POSITION_MARK", &attrs, true);
    }
    xml::end(out, 2, "TRACK");
}

fn write_node(out: &mut String, depth: usize, node: &ExportNode) {
    match node {
        ExportNode::Folder { name, children } => {
            xml::start(out, depth, "NODE", &[("Type", "0".into()), ("Name", name.clone()), ("Count", children.len().to_string())], false);
            for child in children {
                write_node(out, depth + 1, child);
            }
            xml::end(out, depth, "NODE");
        }
        ExportNode::Playlist { name, track_ids } => {
            let attrs = [
                ("Name", name.clone()),
                ("Type", "1".into()),
                ("KeyType", "0".into()),
                ("Entries", track_ids.len().to_string()),
            ];
            xml::start(out, depth, "NODE", &attrs, false);
            for id in track_ids {
                xml::start(out, depth + 1, "TRACK", &[("Key", id.to_string())], true);
            }
            xml::end(out, depth, "NODE");
        }
    }
}

pub fn render(doc: &ExportDocument) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml::start(&mut out, 0, "DJ_PLAYLISTS", &[("Version", "1.0.0".into())], false);
    let product = [
        ("Name", "fourfour".to_string()),
        ("Version", env!("CARGO_PKG_VERSION").to_string()),
        ("Company", String::new()),
    ];
    xml::start(&mut out, 1, "PRODUCT", &product, true);
    xml::start(&mut out, 1, "COLLECTION", &[("Entries", doc.tracks.len().to_string())], false);
    for track in &doc.tracks {
        write_track(&mut out, track);
    }
    xml::end(&mut out, 1, "COLLECTION");
    xml::start(&mut out, 1, "PLAYLISTS", &[], false);
    xml::start(&mut out, 2, "NODE", &[("Type", "0".into()), ("Name", "ROOT".into()), ("Count", "1".into())], false);
    write_node(&mut out, 3, &doc.root);
    xml::end(&mut out, 2, "NODE");
    xml::end(&mut out, 1, "PLAYLISTS");
    xml::end(&mut out, 0, "DJ_PLAYLISTS");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beats(times: &[u32], tempo: u32) -> Vec<Beat> {
        times
            .iter()
            .enumerate()
            .map(|(i, &time_ms)| Beat { bar_position: (i % 4 + 1) as u8, time_ms, tempo })
            .collect()
    }

    #[test]
    fn steady_grid_is_one_tempo_marker() {
        let markers = tempo_markers(&beats(&[100, 600, 1100, 1600, 2100], 12000), 0.0);
        assert_eq!(markers, vec![TempoMarker { start_ms: 100, bpm: 120.0, first_beat_in_bar: 1 }]);
    }

    #[test]
    fn drifting_grid_gets_a_new_marker_where_it_leaves_the_tempo() {
        let markers = tempo_markers(&beats(&[0, 500, 1000, 1500, 2100, 2600], 12000), 0.0);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[1].start_ms, 2100);
        assert_eq!(markers[1].first_beat_in_bar, 1);
    }

    #[test]
    fn locations_are_percent_encoded_file_uris() {
        assert_eq!(location("/Users/me/Music/Tom & Jerry #1.mp3"), "file://localhost/Users/me/Music/Tom%20%26%20Jerry%20%231.mp3");
        assert_eq!(location("C:\\Music\\a.mp3"), "file://localhost/C:/Music/a.mp3");
        assert_eq!(location("/m/Bj\u{f6}rk.mp3"), "file://localhost/m/Bj%C3%B6rk.mp3");
    }
}
