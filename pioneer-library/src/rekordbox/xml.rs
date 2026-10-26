use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

use pioneer_usb_writer::models::{Beat, Track};

use super::model::{ImportedCue, SourceKind, SourceLibrary, SourcePlaylist, SourceTrack};

type Attrs = HashMap<String, String>;

enum Event {
    Start(String, Attrs),
    End(String),
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        let entity = &rest[1..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()));
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => out.push_str(&rest[..=end]),
                }
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn parse_attrs(mut s: &str) -> Attrs {
    let mut attrs = Attrs::new();
    loop {
        s = s.trim_start();
        let Some(eq) = s.find('=') else { break };
        let name = s[..eq].trim().to_string();
        let after = s[eq + 1..].trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
        let Some(close) = after[1..].find(quote) else { break };
        attrs.insert(name, unescape(&after[1..1 + close]));
        s = &after[close + 2..];
    }
    attrs
}

fn tokenize(xml: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        rest = &rest[open..];
        if let Some(body) = rest.strip_prefix("<!--") {
            let Some(end) = body.find("-->") else { break };
            rest = &body[end + 3..];
            continue;
        }
        let Some(close) = rest.find('>') else { break };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            events.push(Event::End(name.trim().to_string()));
            continue;
        }
        let self_closing = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let (name, attr_text) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        events.push(Event::Start(name.to_string(), parse_attrs(attr_text)));
        if self_closing {
            events.push(Event::End(name.to_string()));
        }
    }
    events
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn location_to_path(location: &str) -> String {
    let stripped = location
        .strip_prefix("file://localhost")
        .or_else(|| location.strip_prefix("file://"))
        .unwrap_or(location);
    let decoded = percent_decode(stripped);
    let bytes = decoded.as_bytes();
    let drive_letter = bytes.len() > 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    if drive_letter { decoded[1..].to_string() } else { decoded }
}

fn seconds_to_ms(value: Option<&String>) -> Option<u32> {
    value
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|s| *s >= 0.0)
        .map(|s| (s * 1000.0).round() as u32)
}

fn number<T: std::str::FromStr + Default>(attrs: &Attrs, key: &str) -> T {
    attrs.get(key).and_then(|v| v.parse().ok()).unwrap_or_default()
}

fn colour_hex(attrs: &Attrs) -> Option<String> {
    let parts: Option<Vec<u8>> = ["Red", "Green", "Blue"]
        .iter()
        .map(|k| attrs.get(*k).and_then(|v| v.parse().ok()))
        .collect();
    parts.map(|p| format!("#{:02x}{:02x}{:02x}", p[0], p[1], p[2]))
}

fn parse_cue(attrs: &Attrs) -> Option<ImportedCue> {
    let kind: i32 = number(attrs, "Type");
    if !matches!(kind, 0 | 4) {
        return None;
    }
    let num: i32 = attrs.get("Num").and_then(|v| v.parse().ok()).unwrap_or(-1);
    let start = seconds_to_ms(attrs.get("Start"))?;
    Some(ImportedCue {
        hot_cue_number: if num >= 0 { num as u32 + 1 } else { 0 },
        time_ms: start,
        loop_time_ms: if kind == 4 { seconds_to_ms(attrs.get("End")) } else { None },
        color: colour_hex(attrs),
        name: attrs.get("Name").cloned().unwrap_or_default(),
    })
}

struct TempoEntry {
    start_ms: u32,
    bpm: f64,
    first_beat_in_bar: u8,
}

fn grid_from_tempo(entries: &[TempoEntry], total_ms: u32) -> Vec<Beat> {
    let mut beats = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if entry.bpm <= 0.0 {
            continue;
        }
        let end = entries.get(i + 1).map_or(total_ms, |next| next.start_ms);
        let beat_ms = 60_000.0 / entry.bpm;
        let mut n = 0u32;
        loop {
            let at = entry.start_ms as f64 + n as f64 * beat_ms;
            if at >= end as f64 {
                break;
            }
            beats.push(Beat {
                bar_position: ((entry.first_beat_in_bar as u32 - 1 + n) % 4 + 1) as u8,
                time_ms: at.round() as u32,
                tempo: (entry.bpm * 100.0).round() as u32,
            });
            n += 1;
        }
    }
    beats
}

fn build_track(attrs: &Attrs, tempo: &[TempoEntry], cues: Vec<ImportedCue>) -> SourceTrack {
    let total_secs: u32 = number(attrs, "TotalTime");
    let rating: u32 = number(attrs, "Rating");
    let location = attrs.get("Location").map(|l| location_to_path(l)).unwrap_or_default();
    let grid = grid_from_tempo(tempo, total_secs * 1000);
    SourceTrack {
        source_id: attrs.get("TrackID").cloned().unwrap_or_default(),
        path: location,
        track: Track {
            id: 0,
            source_path: Default::default(),
            usb_path: String::new(),
            title: attrs.get("Name").cloned().unwrap_or_default(),
            artist: attrs.get("Artist").cloned().unwrap_or_default(),
            album: attrs.get("Album").cloned().unwrap_or_default(),
            genre: attrs.get("Genre").cloned().unwrap_or_default(),
            label: attrs.get("Label").cloned().unwrap_or_default(),
            remixer: attrs.get("Remixer").cloned().unwrap_or_default(),
            comment: attrs.get("Comments").cloned().unwrap_or_default(),
            year: number(attrs, "Year"),
            disc_number: number(attrs, "DiscNumber"),
            track_number: number(attrs, "TrackNumber"),
            tempo: (number::<f64>(attrs, "AverageBpm") * 100.0).round() as u32,
            key: attrs.get("Tonality").cloned().unwrap_or_default(),
            duration_secs: total_secs as f64,
            sample_rate: number(attrs, "SampleRate"),
            bitrate: number(attrs, "BitRate"),
            file_size: number(attrs, "Size"),
            artwork: None,
        },
        rating: (rating > 0).then(|| if rating > 5 { (rating / 51).min(5) as u8 } else { rating as u8 }),
        color: attrs
            .get("Colour")
            .map(|c| format!("#{}", c.trim_start_matches("0x").to_lowercase()))
            .filter(|c| c.len() == 7),
        cues,
        anlz_dat: None,
        grid: (!grid.is_empty()).then_some(grid),
        artwork: None,
    }
}

/// Load a `rekordbox.xml` export. Cues and tempo markers come from the file
/// itself; there is no waveform, so the beat grid and cues are kept as imported
/// data until the track is analysed.
pub fn load(xml_path: &Path) -> Result<SourceLibrary> {
    let text = std::fs::read_to_string(xml_path)
        .with_context(|| format!("Failed to read {}", xml_path.display()))?;
    let events = tokenize(&text);
    if !events.iter().any(|e| matches!(e, Event::Start(n, ..) if n == "DJ_PLAYLISTS")) {
        bail!("Not a Rekordbox XML export: no DJ_PLAYLISTS element");
    }

    let mut tracks = Vec::new();
    let mut playlists: Vec<SourcePlaylist> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut node_stack: Vec<usize> = Vec::new();
    let mut current: Option<(Attrs, Vec<TempoEntry>, Vec<ImportedCue>)> = None;

    for event in events {
        match event {
            Event::Start(name, attrs) => {
                let in_collection = stack.iter().any(|s| s == "COLLECTION");
                let in_playlists = stack.iter().any(|s| s == "PLAYLISTS");
                match name.as_str() {
                    "TRACK" if in_collection => current = Some((attrs, Vec::new(), Vec::new())),
                    "TEMPO" => {
                        if let Some((_, tempo, _)) = current.as_mut() {
                            tempo.push(TempoEntry {
                                start_ms: seconds_to_ms(attrs.get("Inizio")).unwrap_or(0),
                                bpm: number(&attrs, "Bpm"),
                                first_beat_in_bar: number::<u8>(&attrs, "Battito").clamp(1, 4),
                            });
                        }
                    }
                    "POSITION_MARK" => {
                        if let (Some((_, _, cues)), Some(cue)) = (current.as_mut(), parse_cue(&attrs)) {
                            cues.push(cue);
                        }
                    }
                    "NODE" if in_playlists => {
                        let index = playlists.len();
                        let is_folder = attrs.get("Type").map(String::as_str) == Some("0");
                        playlists.push(SourcePlaylist {
                            source_id: index.to_string(),
                            parent_id: node_stack.last().map(usize::to_string),
                            name: attrs.get("Name").cloned().unwrap_or_default(),
                            is_folder,
                            sequence: index as i64,
                            track_ids: Vec::new(),
                        });
                        node_stack.push(index);
                    }
                    "TRACK" if in_playlists => {
                        if let (Some(&node), Some(key)) = (node_stack.last(), attrs.get("Key")) {
                            playlists[node].track_ids.push(key.clone());
                        }
                    }
                    _ => {}
                }
                stack.push(name);
            }
            Event::End(name) => {
                stack.pop();
                match name.as_str() {
                    "TRACK" => {
                        if let Some((attrs, tempo, cues)) = current.take() {
                            tracks.push(build_track(&attrs, &tempo, cues));
                        }
                    }
                    "NODE" => {
                        node_stack.pop();
                    }
                    _ => {}
                }
            }
        }
    }

    // The invisible ROOT node is not a folder the user sees.
    let root = playlists.iter().position(|p| p.parent_id.is_none() && p.is_folder && p.name == "ROOT");
    if let Some(root) = root {
        let root_id = playlists[root].source_id.clone();
        playlists.remove(root);
        for p in &mut playlists {
            if p.parent_id.as_deref() == Some(root_id.as_str()) {
                p.parent_id = None;
            }
        }
    }

    Ok(SourceLibrary {
        kind: SourceKind::Xml,
        tracks,
        playlists,
        implied_remap: None,
        skipped_smart_playlists: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.0.0" Company="AlphaTheta"/>
  <COLLECTION Entries="2">
    <TRACK TrackID="1" Name="Tom &amp; Jerry" Artist="Cat" Album="A" Genre="House" TotalTime="10" AverageBpm="120.00" Year="2020" Rating="102" Tonality="8A" Colour="0xFF007F" Size="1000" Location="file://localhost/Users/me/Music/House/Tom%20%26%20Jerry.mp3">
      <TEMPO Inizio="0.500" Bpm="120.00" Metro="4/4" Battito="1"/>
      <POSITION_MARK Name="Drop" Type="0" Start="1.000" Num="0" Red="40" Green="226" Blue="20"/>
      <POSITION_MARK Name="" Type="0" Start="2.000" Num="-1"/>
      <POSITION_MARK Name="Loop" Type="4" Start="4.000" End="6.000" Num="1" Red="1" Green="2" Blue="3"/>
    </TRACK>
    <TRACK TrackID="2" Name="Two" Location="file://localhost/C:/Music/two.mp3"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="2">
      <NODE Type="0" Name="Sets" Count="1">
        <NODE Name="Friday" Type="1" KeyType="0" Entries="2"><TRACK Key="2"/><TRACK Key="1"/></NODE>
      </NODE>
      <NODE Name="Top" Type="1" KeyType="0" Entries="1"><TRACK Key="1"/></NODE>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>"#;

    fn parsed() -> SourceLibrary {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rekordbox.xml");
        std::fs::write(&path, SAMPLE).unwrap();
        load(&path).unwrap()
    }

    #[test]
    fn reads_tracks_with_decoded_paths_and_tags() {
        let lib = parsed();
        assert_eq!(lib.tracks.len(), 2);
        let t = &lib.tracks[0];
        assert_eq!(t.path, "/Users/me/Music/House/Tom & Jerry.mp3");
        assert_eq!(t.track.title, "Tom & Jerry");
        assert_eq!(t.track.tempo, 12000);
        assert_eq!(t.track.key, "8A");
        assert_eq!(t.rating, Some(2));
        assert_eq!(t.color.as_deref(), Some("#ff007f"));
        assert_eq!(lib.tracks[1].path, "C:/Music/two.mp3");
    }

    #[test]
    fn reads_cues_with_colours_and_memory_flag() {
        let cues = &parsed().tracks[0].cues;
        assert_eq!(cues.len(), 3);
        assert_eq!((cues[0].hot_cue_number, cues[0].time_ms, cues[0].name.as_str()), (1, 1000, "Drop"));
        assert_eq!(cues[0].color.as_deref(), Some("#28e214"));
        assert_eq!(cues[1].hot_cue_number, 0);
        assert_eq!(cues[2].loop_time_ms, Some(6000));
    }

    #[test]
    fn builds_beat_grid_from_tempo() {
        let grid = parsed().tracks[0].grid.clone().unwrap();
        assert_eq!(grid[0].time_ms, 500);
        assert_eq!(grid[1].time_ms, 1000);
        assert_eq!(grid[4].bar_position, 1);
        assert_eq!(grid[0].tempo, 12000);
    }

    #[test]
    fn nested_playlists_keep_parents_and_drop_root() {
        let lib = parsed();
        let friday = lib.playlists.iter().find(|p| p.name == "Friday").unwrap();
        let sets = lib.playlists.iter().find(|p| p.name == "Sets").unwrap();
        assert_eq!(friday.parent_id.as_deref(), Some(sets.source_id.as_str()));
        assert_eq!(friday.track_ids, vec!["2", "1"]);
        assert!(sets.parent_id.is_none());
        assert!(lib.playlists.iter().all(|p| p.name != "ROOT"));
    }
}
