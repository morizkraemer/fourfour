use std::fmt::Write;

use super::ExportTrack;
use super::xml::file_name;

fn single_line(text: &str) -> String {
    text.split(['\r', '\n']).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

fn tracklist_line(track: &ExportTrack) -> String {
    let title = single_line(&track.track.title);
    let artist = single_line(&track.track.artist);
    match (artist.is_empty(), title.is_empty()) {
        (false, false) => format!("{artist} - {title}"),
        (true, false) => title,
        (false, true) => artist,
        (true, true) => file_name(&track.path).to_string(),
    }
}

fn duration_label(seconds: f64) -> String {
    let total = seconds.round() as u64;
    if total >= 3600 {
        format!("{}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

pub fn text(tracks: &[&ExportTrack]) -> String {
    tracks.iter().map(|t| format!("{}\n", tracklist_line(t))).collect()
}

pub fn m3u8(tracks: &[&ExportTrack]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for track in tracks {
        let seconds = if track.track.duration_secs > 0.0 { track.track.duration_secs.round() as i64 } else { -1 };
        let _ = writeln!(out, "#EXTINF:{seconds},{}", tracklist_line(track));
        let _ = writeln!(out, "{}", single_line(&track.path));
    }
    out
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

pub fn csv(tracks: &[&ExportTrack]) -> String {
    let mut out = String::from("position,title,artist,album,bpm,key,duration,path\r\n");
    for (index, track) in tracks.iter().enumerate() {
        let bpm = if track.bpm > 0.0 { format!("{:.2}", track.bpm) } else { String::new() };
        let duration = if track.track.duration_secs > 0.0 { duration_label(track.track.duration_secs) } else { String::new() };
        let fields = [
            (index + 1).to_string(),
            csv_field(&track.track.title),
            csv_field(&track.track.artist),
            csv_field(&track.track.album),
            bpm,
            csv_field(&track.key),
            duration,
            csv_field(&track.path),
        ];
        out.push_str(&fields.join(","));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_as_minutes_and_seconds() {
        assert_eq!(duration_label(61.4), "1:01");
        assert_eq!(duration_label(3725.0), "1:02:05");
    }

    #[test]
    fn csv_quotes_commas_quotes_and_line_breaks() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("plain"), "plain");
    }
}
