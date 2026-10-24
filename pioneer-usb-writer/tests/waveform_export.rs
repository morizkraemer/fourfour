use std::path::Path;

use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, Playlist, Track, WaveformPreview,
};
use pioneer_usb_writer::reader::anlz::read_anlz;
use pioneer_usb_writer::waveform::{
    Meter, OVERVIEW_COLUMNS, PWAV_COLUMNS, PWV2_COLUMNS, PWV6_MAX, PWV7_MAX, Waveforms, analyze,
};
use pioneer_usb_writer::writer::anlz::{
    anlz_2ex_path_for_track, anlz_ext_path_for_track, anlz_path_for_track, write_anlz_2ex,
};
use pioneer_usb_writer::writer::filesystem::{sync_usb, write_usb};
use tempfile::TempDir;

const FILE_HEADER_LEN: usize = 0x1C;

const DAT_ORDER: [&str; 7] = ["PPTH", "PVBR", "PQTZ", "PWAV", "PWV2", "PCOB", "PCOB"];
const EXT_ORDER: [&str; 10] = [
    "PPTH", "PWV3", "PCOB", "PCOB", "PCO2", "PCO2", "PQT2", "PWV5", "PWV4", "PVB2",
];
const TWO_EX_ORDER: [&str; 4] = ["PPTH", "PWV7", "PWV6", "PWVC"];

fn write_wav(path: &Path) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::write(path, bytes).unwrap();
}

fn track(source_dir: &Path, id: u32, duration_secs: f64) -> Track {
    let file = format!("track{id}.wav");
    let source_path = source_dir.join(&file);
    write_wav(&source_path);
    Track {
        usb_path: format!("/Contents/Artist/{file}"),
        source_path: source_path.clone(),
        title: format!("Track {id}"),
        artist: "Artist".into(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: id,
        tempo: 12800,
        key: "8A".into(),
        duration_secs,
        sample_rate: 8000,
        bitrate: 128,
        file_size: std::fs::metadata(&source_path).unwrap().len(),
        id,
        artwork: None,
    }
}

fn synthetic_detail(columns: usize, seed: usize) -> Vec<[u8; 3]> {
    (0..columns)
        .map(|i| {
            let swell = ((i / 150 + seed) % 8) as u8 * 15 + 7;
            [
                ((i * 7 + seed) % 128) as u8,
                (swell as usize + i % 11).min(127) as u8,
                ((i * 3) % 101) as u8,
            ]
        })
        .collect()
}

fn analysis(color_waveform: Option<ColorWaveform>) -> AnalysisResult {
    let mut data = [0u8; 400];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i % 30) as u8 + 2 | 0xA0;
    }
    AnalysisResult {
        beat_grid: BeatGrid { beats: vec![Beat { bar_position: 1, time_ms: 0, tempo: 12800 }] },
        waveform: WaveformPreview { data },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: vec![],
        color_waveform,
    }
}

fn banded(duration_secs: f64, seed: usize) -> AnalysisResult {
    let columns = (duration_secs * 150.0).round() as usize;
    analysis(Some(ColorWaveform {
        detail: synthetic_detail(columns, seed),
        overview: vec![[1, 2, 3]; OVERVIEW_COLUMNS],
    }))
}

fn tags(file: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = FILE_HEADER_LEN;
    while at + 12 <= file.len() {
        let total = u32::from_be_bytes(file[at + 8..at + 12].try_into().unwrap()) as usize;
        out.push(String::from_utf8_lossy(&file[at..at + 4]).to_string());
        at += total;
    }
    assert_eq!(at, file.len(), "sections must tile the file exactly");
    out
}

fn section<'a>(file: &'a [u8], tag: &str) -> &'a [u8] {
    let mut at = FILE_HEADER_LEN;
    while at + 12 <= file.len() {
        let total = u32::from_be_bytes(file[at + 8..at + 12].try_into().unwrap()) as usize;
        if &file[at..at + 4] == tag.as_bytes() {
            return &file[at..at + total];
        }
        at += total;
    }
    panic!("section {tag} missing");
}

fn be32(b: &[u8], at: usize) -> usize {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

fn assert_order(actual: &[String], expected: &[&str]) {
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    let mut wanted = expected.to_vec();
    wanted.retain(|t| actual.contains(t));
    assert_eq!(
        actual.iter().filter(|t| wanted.contains(t)).count(),
        actual.len(),
        "unexpected sections in {actual:?}"
    );
    let mut cursor = 0;
    for tag in &actual {
        let pos = expected[cursor..].iter().position(|e| e == tag).unwrap_or_else(|| {
            panic!("section {tag} out of order in {actual:?}, expected order {expected:?}")
        });
        cursor += pos + 1;
    }
}

fn exported_tracks(durations: &[f64]) -> (TempDir, TempDir, Vec<Track>, Vec<AnalysisResult>) {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let tracks: Vec<Track> =
        durations.iter().enumerate().map(|(i, d)| track(source.path(), i as u32 + 1, *d)).collect();
    let analyses: Vec<AnalysisResult> =
        durations.iter().enumerate().map(|(i, d)| banded(*d, i)).collect();
    (source, usb, tracks, analyses)
}

fn assert_bands_in_range(two_ex: &[u8], ext: &[u8], columns: usize) {
    let pwv7 = section(two_ex, "PWV7");
    assert_eq!(pwv7.len(), 24 + columns * 3);
    assert_eq!(be32(pwv7, 16), columns);
    assert!(pwv7[24..].iter().all(|&b| b <= PWV7_MAX), "PWV7 byte above {PWV7_MAX}");

    let pwv6 = section(two_ex, "PWV6");
    assert_eq!(pwv6.len(), 20 + OVERVIEW_COLUMNS * 3);
    assert_eq!(be32(pwv6, 16), OVERVIEW_COLUMNS);
    assert!(pwv6[20..].iter().all(|&b| b <= PWV6_MAX), "PWV6 byte above {PWV6_MAX}");

    let pwvc = section(two_ex, "PWVC");
    assert_eq!(pwvc.len(), 20);

    let pwv5 = section(ext, "PWV5");
    assert_eq!(pwv5.len(), 24 + columns * 2);
    assert!(pwv5[24..].chunks_exact(2).all(|c| c[1] & 0b11 == 0), "PWV5 low two bits must stay clear");
    let pwv3 = section(ext, "PWV3");
    assert_eq!(pwv3.len(), 24 + columns);
    for i in 0..columns {
        let v5 = u16::from_be_bytes([pwv5[24 + i * 2], pwv5[25 + i * 2]]);
        assert_eq!(((v5 >> 2) & 31) as u8, pwv3[24 + i] & 31, "PWV3 and PWV5 heights differ at {i}");
    }
    let pwv4 = section(ext, "PWV4");
    assert_eq!(pwv4.len(), 24 + OVERVIEW_COLUMNS * 6);
    assert!(
        pwv4[24..].chunks_exact(6).all(|c| c[2..6].iter().all(|&b| b <= PWV7_MAX)),
        "PWV4 height and band bytes must stay at most {PWV7_MAX}"
    );
}

#[test]
fn export_roundtrips_band_data_for_1_7_10_20_and_60_minute_tracks() {
    let durations = [60.0, 437.0, 600.0, 1200.0, 3600.0];
    let (_source, usb, tracks, analyses) = exported_tracks(&durations);
    write_usb(usb.path(), &tracks, &analyses, &[Playlist { id: 1, name: "All".into(), track_ids: vec![1, 2, 3, 4, 5] }])
        .unwrap();

    for ((t, a), duration) in tracks.iter().zip(&analyses).zip(durations) {
        let columns = (duration * 150.0).round() as usize;
        let dat_path = usb.path().join(anlz_path_for_track(t));
        let ext_path = usb.path().join(anlz_ext_path_for_track(t));
        let two_ex_path = usb.path().join(anlz_2ex_path_for_track(t));
        assert!(two_ex_path.is_file(), "{duration}s: .2EX missing");

        let dat = std::fs::read(&dat_path).unwrap();
        let ext = std::fs::read(&ext_path).unwrap();
        let two_ex = std::fs::read(&two_ex_path).unwrap();
        assert_order(&tags(&dat), &DAT_ORDER);
        assert_order(&tags(&ext), &EXT_ORDER);
        assert_eq!(tags(&two_ex), TWO_EX_ORDER, "{duration}s: .2EX section order");
        assert!(
            !tags(&ext).iter().any(|t| ["PWV7", "PWV6", "PWVC"].contains(&t.as_str())),
            "{duration}s: 3-band sections belong in .2EX, not .EXT"
        );
        assert_eq!(section(&dat, "PWAV").len(), 20 + PWAV_COLUMNS);
        assert_eq!(section(&dat, "PWV2").len(), 20 + PWV2_COLUMNS);
        assert_bands_in_range(&two_ex, &ext, columns);

        let read_back = read_anlz(&dat_path).unwrap().color_waveform.expect("color waveform read back");
        assert_eq!(read_back.detail.len(), columns, "{duration}s: detail length");
        assert_eq!(read_back.overview.len(), OVERVIEW_COLUMNS, "{duration}s: overview length");
        assert_eq!(read_back.detail, a.color_waveform.as_ref().unwrap().detail, "{duration}s: detail bytes");
        assert!(read_back.detail.iter().flatten().all(|&b| b <= PWV7_MAX));
        assert!(read_back.overview.iter().flatten().all(|&b| b <= PWV6_MAX));
    }
}

#[test]
fn band_bytes_above_127_are_clamped() {
    let source = TempDir::new().unwrap();
    let t = track(source.path(), 1, 10.0);
    let a = analysis(Some(ColorWaveform { detail: vec![[255, 200, 128]; 1500], overview: Vec::new() }));
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ANLZ0000.2EX");
    assert!(write_anlz_2ex(&path, &t, &a).unwrap());
    let file = std::fs::read(&path).unwrap();
    assert!(section(&file, "PWV7")[24..].iter().all(|&b| b == 127));
}

#[test]
fn analysis_without_bands_writes_neutral_colour_and_no_2ex() {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let t = track(source.path(), 1, 30.0);
    write_usb(usb.path(), &[t.clone()], &[analysis(None)], &[]).unwrap();

    assert!(!usb.path().join(anlz_2ex_path_for_track(&t)).exists());
    let ext = std::fs::read(usb.path().join(anlz_ext_path_for_track(&t))).unwrap();
    assert_order(&tags(&ext), &EXT_ORDER);
    let pwv5 = section(&ext, "PWV5");
    assert!(
        pwv5[24..].chunks_exact(2).all(|c| {
            let v = u16::from_be_bytes([c[0], c[1]]);
            (v >> 13) & 7 == 7 && (v >> 10) & 7 == 7 && (v >> 7) & 7 == 7
        }),
        "without bands every PWV5 column carries equal levels"
    );
    let read_back = read_anlz(&usb.path().join(anlz_path_for_track(&t))).unwrap();
    assert_eq!(read_back.waveform.data, analysis(None).waveform.data);
}

#[test]
fn two_ex_is_written_listed_synced_and_removed() {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let t1 = track(source.path(), 1, 20.0);
    let t2 = track(source.path(), 2, 25.0);
    let tracks = vec![t1.clone(), t2.clone()];
    let analyses = vec![banded(20.0, 1), banded(25.0, 2)];

    let first = sync_usb(usb.path(), &tracks, &analyses, &[]).unwrap();
    assert_eq!(first.tracks_added, 2);
    for t in &tracks {
        let dir = usb.path().join(anlz_path_for_track(t)).parent().unwrap().to_path_buf();
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["ANLZ0000.2EX", "ANLZ0000.DAT", "ANLZ0000.EXT"]);
    }

    let before = std::fs::read(usb.path().join(anlz_2ex_path_for_track(&t1))).unwrap();
    let mut renamed = tracks.clone();
    renamed[0].title = "Renamed".into();
    let changed = vec![banded(20.0, 9), analyses[1].clone()];
    let second = sync_usb(usb.path(), &renamed, &changed, &[]).unwrap();
    assert_eq!(second.tracks_updated, 1, "title change must rewrite the ANLZ files of that track");
    let after = std::fs::read(usb.path().join(anlz_2ex_path_for_track(&t1))).unwrap();
    assert_ne!(before, after, "update must rewrite .2EX with the new bands");

    let lost_bands = vec![analysis(None), analyses[1].clone()];
    renamed[0].title = "Renamed again".into();
    sync_usb(usb.path(), &renamed, &lost_bands, &[]).unwrap();
    assert!(!usb.path().join(anlz_2ex_path_for_track(&t1)).exists(), "stale .2EX must go");
    assert!(usb.path().join(anlz_2ex_path_for_track(&t2)).is_file());

    let dropped = sync_usb(usb.path(), &renamed[1..], &lost_bands[1..], &[]).unwrap();
    assert_eq!(dropped.tracks_removed, 1);
    assert!(!usb.path().join(anlz_path_for_track(&t1)).parent().unwrap().exists());
    assert!(usb.path().join(anlz_2ex_path_for_track(&t2)).is_file());
}

fn tone(rate: u32, seconds: f64, hz: f64, amp: f64) -> Vec<f32> {
    (0..(rate as f64 * seconds) as usize)
        .map(|i| (amp * (std::f64::consts::TAU * hz * i as f64 / rate as f64).sin()) as f32)
        .collect()
}

#[test]
fn analyzed_audio_exports_and_reads_back_identically() {
    let rate = 22050;
    let mut samples = tone(rate, 6.0, 60.0, 0.9);
    samples.extend(tone(rate, 6.0, 1000.0, 0.5));
    samples.extend(tone(rate, 6.0, 9000.0, 0.3));
    let mut meter = Meter::new(rate, 1);
    meter.push(&samples);
    let waves: Waveforms = analyze(&meter.finish());
    let duration_secs = waves.pwv7.len() as f64 / 150.0;

    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let t = track(source.path(), 1, duration_secs);
    let mut a = analysis(Some(waves.color_waveform()));
    a.waveform = waves.preview().unwrap();
    write_usb(usb.path(), &[t.clone()], &[a], &[]).unwrap();

    let read_back = read_anlz(&usb.path().join(anlz_path_for_track(&t))).unwrap();
    let cw = read_back.color_waveform.unwrap();
    assert_eq!(cw.detail, waves.pwv7);
    assert_eq!(cw.overview.len(), OVERVIEW_COLUMNS);
    let ext = std::fs::read(usb.path().join(anlz_ext_path_for_track(&t))).unwrap();
    let two_ex = std::fs::read(usb.path().join(anlz_2ex_path_for_track(&t))).unwrap();
    assert_bands_in_range(&two_ex, &ext, waves.pwv7.len());

    let bass = &cw.detail[3 * 150];
    let mid = &cw.detail[9 * 150];
    let treble = &cw.detail[15 * 150];
    assert!(bass[0] > bass[1] && bass[0] > bass[2], "bass section: {bass:?}");
    assert!(mid[1] > mid[0] && mid[1] > mid[2], "mid section: {mid:?}");
    assert!(treble[2] > treble[0] && treble[2] > treble[1], "treble section: {treble:?}");
}
