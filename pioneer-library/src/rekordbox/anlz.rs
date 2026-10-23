use std::collections::HashMap;
use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, CuePoint};
use pioneer_usb_writer::reader;

use super::model::{ImportProgress, ImportedCue, SourceLibrary};

pub struct LoadedAnalysis {
    pub analysis: AnalysisResult,
    pub cues: Vec<ImportedCue>,
}

fn be_u32(data: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn be_u16(data: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn utf16be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Cues from the PCO2 sections of an ANLZ `.EXT` file, including the comment
/// and colour that the writer crate's reader drops. Entries are walked by their
/// own length field because comments make them variable-sized.
pub fn read_ext_cues(ext_path: &Path) -> Vec<ImportedCue> {
    let Ok(data) = std::fs::read(ext_path) else {
        return Vec::new();
    };
    if data.len() < 28 || &data[0..4] != b"PMAI" {
        return Vec::new();
    }
    let mut cues = Vec::new();
    let mut offset = be_u32(&data, 4).unwrap_or(data.len());
    while offset + 12 <= data.len() {
        let Some(section_len) = be_u32(&data, offset + 8) else { break };
        if section_len == 0 || offset + section_len > data.len() {
            break;
        }
        if &data[offset..offset + 4] == b"PCO2" {
            let section = &data[offset..offset + section_len];
            let count = be_u16(section, 16).unwrap_or(0);
            let mut at = be_u32(section, 4).unwrap_or(section.len());
            for _ in 0..count {
                let Some(entry_len) = be_u32(section, at + 8) else { break };
                if entry_len < 0x2c || at + entry_len > section.len() {
                    break;
                }
                let entry = &section[at..at + entry_len];
                if &entry[0..4] == b"PCP2" {
                    cues.push(parse_pcp2(entry));
                }
                at += entry_len;
            }
        }
        offset += section_len;
    }
    cues
}

fn parse_pcp2(entry: &[u8]) -> ImportedCue {
    let loop_raw = be_u32(entry, 24).unwrap_or(0xFFFF_FFFF);
    let comment_len = be_u32(entry, 0x28).unwrap_or(0);
    let comment_end = (0x2c + comment_len).min(entry.len());
    let name = utf16be(&entry[0x2c.min(comment_end)..comment_end]);
    let color = entry.get(comment_end..comment_end + 4).and_then(|c| {
        let (r, g, b) = (c[1], c[2], c[3]);
        (r | g | b != 0).then(|| format!("#{r:02x}{g:02x}{b:02x}"))
    });
    ImportedCue {
        hot_cue_number: be_u32(entry, 12).unwrap_or(0) as u32,
        time_ms: be_u32(entry, 20).unwrap_or(0) as u32,
        loop_time_ms: (loop_raw != 0xFFFF_FFFF).then_some(loop_raw as u32),
        color,
        name,
    }
}

pub fn to_cue_points(cues: &[ImportedCue]) -> Vec<CuePoint> {
    cues.iter()
        .map(|c| CuePoint {
            hot_cue_number: c.hot_cue_number,
            time_ms: c.time_ms,
            loop_time_ms: c.loop_time_ms,
        })
        .collect()
}

/// The writer crate indexes the colour detail as `i * len / entries` in `u32`,
/// which overflows for long tracks with Rekordbox's 150-entries-per-second
/// detail. Decimating the source (keeping the loudest sample per group) keeps
/// the product below `u32::MAX` until that is fixed in the writer.
fn fit_color_detail_for_writer(analysis: &mut AnalysisResult, duration_secs: f64) {
    let entries = (duration_secs * 150.0).round().max(0.0) as u64;
    let Some(waveform) = analysis.color_waveform.as_mut() else { return };
    if entries == 0 || waveform.detail.len() as u64 * entries < u32::MAX as u64 {
        return;
    }
    let max_len = ((u32::MAX as u64 / entries).saturating_sub(1)).max(1) as usize;
    let group = waveform.detail.len().div_ceil(max_len);
    waveform.detail = waveform
        .detail
        .chunks(group)
        .map(|chunk| {
            chunk.iter().fold([0u8; 3], |acc, e| [acc[0].max(e[0]), acc[1].max(e[1]), acc[2].max(e[2])])
        })
        .collect();
}

fn load_one(dat: &Path, source_cues: &[ImportedCue], tempo_x100: u32, key: &str, duration_secs: f64) -> Option<LoadedAnalysis> {
    let mut analysis = reader::anlz::read_anlz(dat).ok()?;
    let cues = if !source_cues.is_empty() {
        source_cues.to_vec()
    } else {
        let ext_cues = read_ext_cues(&dat.with_extension("EXT"));
        if ext_cues.is_empty() {
            analysis
                .cue_points
                .iter()
                .map(|c| ImportedCue {
                    hot_cue_number: c.hot_cue_number,
                    time_ms: c.time_ms,
                    loop_time_ms: c.loop_time_ms,
                    color: None,
                    name: String::new(),
                })
                .collect()
        } else {
            ext_cues
        }
    };
    analysis.cue_points = to_cue_points(&cues);
    fit_color_detail_for_writer(&mut analysis, duration_secs);
    analysis.key = key.to_string();
    if tempo_x100 > 0 {
        analysis.bpm = tempo_x100 as f64 / 100.0;
    }
    Some(LoadedAnalysis { analysis, cues })
}

/// Parse every referenced ANLZ file in parallel, keyed by source track id.
/// Tracks without a readable file are simply absent from the result.
pub fn load_all(
    source: &SourceLibrary,
    progress: &(dyn Fn(ImportProgress) + Sync),
) -> HashMap<String, LoadedAnalysis> {
    let tracks: Vec<_> = source.tracks.iter().filter(|t| t.anlz_dat.is_some()).collect();
    let total = tracks.len() as u32;
    let done = std::sync::atomic::AtomicU32::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = tracks.len().div_ceil(workers).max(1);

    let mut out = HashMap::with_capacity(tracks.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = tracks
            .chunks(chunk)
            .map(|part| {
                let done = &done;
                scope.spawn(move || {
                    part.iter()
                        .filter_map(|t| {
                            let loaded = load_one(
                                t.anlz_dat.as_ref()?,
                                &t.cues,
                                t.track.tempo,
                                &t.track.key,
                                t.track.duration_secs,
                            );
                            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            if n % 25 == 0 || n == total {
                                progress(ImportProgress { stage: "analysis", done: n, total });
                            }
                            loaded.map(|l| (t.source_id.clone(), l))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("ANLZ reader thread panicked"));
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcp2(hot_cue: u32, time_ms: u32, loop_ms: u32, comment: &str, rgb: [u8; 3]) -> Vec<u8> {
        let comment_bytes: Vec<u8> = comment
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_be_bytes)
            .collect();
        let len = 0x2c + comment_bytes.len() + 4;
        let mut entry = vec![0u8; len];
        entry[0..4].copy_from_slice(b"PCP2");
        entry[4..8].copy_from_slice(&0x10u32.to_be_bytes());
        entry[8..12].copy_from_slice(&(len as u32).to_be_bytes());
        entry[12..16].copy_from_slice(&hot_cue.to_be_bytes());
        entry[20..24].copy_from_slice(&time_ms.to_be_bytes());
        entry[24..28].copy_from_slice(&loop_ms.to_be_bytes());
        entry[0x28..0x2c].copy_from_slice(&(comment_bytes.len() as u32).to_be_bytes());
        entry[0x2c..0x2c + comment_bytes.len()].copy_from_slice(&comment_bytes);
        let at = 0x2c + comment_bytes.len();
        entry[at] = 1;
        entry[at + 1..at + 4].copy_from_slice(&rgb);
        entry
    }

    fn ext_file(entries: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = entries.concat();
        let section_len = 20 + body.len();
        let mut section = vec![0u8; 20];
        section[0..4].copy_from_slice(b"PCO2");
        section[4..8].copy_from_slice(&20u32.to_be_bytes());
        section[8..12].copy_from_slice(&(section_len as u32).to_be_bytes());
        section[12..16].copy_from_slice(&1u32.to_be_bytes());
        section[16..18].copy_from_slice(&(entries.len() as u16).to_be_bytes());
        section.extend(body);
        let mut file = vec![0u8; 28];
        file[0..4].copy_from_slice(b"PMAI");
        file[4..8].copy_from_slice(&28u32.to_be_bytes());
        file.extend(section);
        let total = file.len() as u32;
        file[8..12].copy_from_slice(&total.to_be_bytes());
        file
    }

    #[test]
    fn pco2_entries_of_different_lengths_keep_names_and_colours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ANLZ0000.EXT");
        std::fs::write(
            &path,
            ext_file(&[
                pcp2(1, 1000, 0xFFFF_FFFF, "Drop", [40, 226, 20]),
                pcp2(0, 2000, 0xFFFF_FFFF, "", [0, 0, 0]),
                pcp2(2, 3000, 4000, "A longer comment", [255, 0, 128]),
            ]),
        )
        .unwrap();

        let cues = read_ext_cues(&path);

        assert_eq!(cues.len(), 3);
        assert_eq!((cues[0].hot_cue_number, cues[0].name.as_str()), (1, "Drop"));
        assert_eq!(cues[0].color.as_deref(), Some("#28e214"));
        assert_eq!((cues[1].hot_cue_number, cues[1].color.clone()), (0, None));
        assert_eq!(cues[2].loop_time_ms, Some(4000));
        assert_eq!(cues[2].name, "A longer comment");
        assert_eq!(cues[2].color.as_deref(), Some("#ff0080"));
    }

    #[test]
    fn long_track_colour_detail_is_decimated_below_writer_overflow() {
        let mut analysis = AnalysisResult {
            beat_grid: pioneer_usb_writer::models::BeatGrid { beats: Vec::new() },
            waveform: pioneer_usb_writer::models::WaveformPreview { data: [0; 400] },
            bpm: 0.0,
            key: String::new(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: Some(pioneer_usb_writer::models::ColorWaveform {
                detail: vec![[1, 2, 3]; 90_000],
                overview: Vec::new(),
            }),
        };
        fit_color_detail_for_writer(&mut analysis, 600.0);
        let len = analysis.color_waveform.unwrap().detail.len() as u64;
        assert!(len * 90_000 < u32::MAX as u64);
    }
}
