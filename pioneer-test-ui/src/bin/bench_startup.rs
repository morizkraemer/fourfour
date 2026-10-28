#[allow(dead_code)]
#[path = "../analysis_payload.rs"]
mod analysis_payload;
#[allow(dead_code)]
#[path = "../dto.rs"]
mod dto;
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;
#[path = "../track_infos.rs"]
mod track_infos;

use std::path::{Path, PathBuf};
use std::time::Instant;

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, Track, WaveformPreview,
};

use dto::{LoadedState, PlaylistInput};

const TRACK_SECS: f64 = 240.0;

fn fake_track(i: usize) -> Track {
    Track {
        id: 0,
        source_path: PathBuf::from(format!("/music/bench/artist{}/track{:06}.mp3", i % 400, i)),
        usb_path: format!("/Contents/Artist{}/track{:06}.mp3", i % 400, i),
        title: format!("Benchmark Track {i}"),
        artist: format!("Artist {}", i % 400),
        album: format!("Album {}", i % 1500),
        genre: ["House", "Techno", "Drum & Bass", "Disco", "Ambient"][i % 5].to_string(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2000 + (i % 25) as u16,
        disc_number: 1,
        track_number: (i % 12) as u32 + 1,
        tempo: 9000 + (i % 80) as u32 * 100,
        key: format!("{}{}", i % 12 + 1, if i % 2 == 0 { "A" } else { "B" }),
        duration_secs: TRACK_SECS,
        sample_rate: 44100,
        bitrate: 320,
        file_size: 9_600_000,
        artwork: if i % 3 == 0 { Some(vec![0xFF; 64]) } else { None },
    }
}

fn fake_analysis(i: usize) -> AnalysisResult {
    let bpm = 90.0 + (i % 80) as f64;
    let beat_ms = (60_000.0 / bpm) as u32;
    let beats = (0..(TRACK_SECS * bpm / 60.0) as u32)
        .map(|b| Beat {
            bar_position: (b % 4) as u8 + 1,
            time_ms: b * beat_ms,
            tempo: (bpm * 100.0) as u32,
        })
        .collect();
    let mut data = [0u8; 400];
    for (n, byte) in data.iter_mut().enumerate() {
        *byte = (((n * 7 + i) % 31) as u8 + 1) | (((n + i) % 8) as u8) << 5;
    }
    let detail_len = (TRACK_SECS * 150.0) as usize;
    let detail = (0..detail_len)
        .map(|n| [((n + i) % 120) as u8, ((n * 3) % 100) as u8, ((n * 5 + i) % 90) as u8])
        .collect();
    let overview = (0..1200)
        .map(|n| [(n % 120) as u8, (n % 100) as u8, (n % 90) as u8])
        .collect();
    AnalysisResult {
        beat_grid: BeatGrid { beats },
        waveform: WaveformPreview { data },
        bpm,
        key: format!("{}{}", i % 12 + 1, if i % 2 == 0 { "A" } else { "B" }),
        energy: Some((i % 10) as u8),
        cue_points: Vec::new(),
        color_waveform: Some(ColorWaveform { detail, overview, exact: None }),
    }
}

fn generate(dir: &Path, count: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let tracks: Vec<Track> = (0..count).map(fake_track).collect();
    let ids = lib.add_tracks(&tracks).unwrap();
    for (n, id) in ids.iter().enumerate() {
        lib.set_analysis(*id, &fake_analysis(n)).unwrap();
        if n % 2000 == 0 {
            eprintln!("analysed {n}/{count}");
        }
    }
    for p in 0..20 {
        let pid = lib.create_playlist(&format!("Playlist {p}")).unwrap();
        let members: Vec<i64> = ids.iter().skip(p * 7).step_by(count / 100 + 1).copied().collect();
        lib.set_playlist_tracks(pid, &members).unwrap();
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn run(dir: &Path) {
    let t = Instant::now();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let open_ms = ms(t);

    let t = Instant::now();
    let rows = lib.get_all_tracks_with_flags().unwrap();
    let rows_ms = ms(t);
    drop(rows);

    let t = Instant::now();
    let infos = track_infos::build_track_infos(&lib).unwrap();
    let build_ms = ms(t);

    let t = Instant::now();
    let playlists: Vec<PlaylistInput> = lib
        .get_all_playlists()
        .unwrap()
        .into_iter()
        .map(|pl| PlaylistInput { id: pl.id, name: pl.name, track_ids: pl.track_ids, parent_id: pl.parent_id, is_folder: pl.is_folder })
        .collect();
    let playlists_ms = ms(t);

    let count = infos.len();
    let t = Instant::now();
    let json = serde_json::to_string(&LoadedState { tracks: infos, playlists }).unwrap();
    let json_ms = ms(t);

    println!(
        "tracks={count} open_ms={open_ms:.1} rows_ms={rows_ms:.1} build_track_infos_ms={build_ms:.1} playlists_ms={playlists_ms:.1} json_ms={json_ms:.1} json_bytes={} load_state_total_ms={:.1}",
        json.len(),
        build_ms + playlists_ms + json_ms,
    );
}

fn median_and_p95(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[(v.len() * 95 / 100).min(v.len() - 1)])
}

fn load(dir: &Path, samples: usize) {
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let ids = lib.get_unanalyzed_track_ids().unwrap();
    let total = lib.track_count().unwrap() as usize;
    drop(ids);
    let step = (total / samples).max(1);
    let picked: Vec<i64> = (0..samples).map(|n| (n * step + 1) as i64).collect();

    let mut parse = Vec::new();
    let mut build = Vec::new();
    let mut json = Vec::new();
    let mut bytes = Vec::new();
    for pass in 0..2 {
        for id in &picked {
            let t = Instant::now();
            let analysis = lib.get_analysis(*id).unwrap().unwrap();
            let parse_ms = ms(t);
            drop(analysis);

            let t = Instant::now();
            let value = analysis_payload::build_analysis_json(&lib, *id).unwrap();
            let build_ms = ms(t);

            let t = Instant::now();
            let text = serde_json::to_string(&value).unwrap();
            let json_ms = ms(t);
            if pass == 1 {
                parse.push(parse_ms);
                build.push(build_ms);
                json.push(json_ms);
                bytes.push(text.len() as f64);
            }
        }
    }
    let (parse_m, parse_p) = median_and_p95(parse);
    let (build_m, build_p) = median_and_p95(build);
    let (json_m, json_p) = median_and_p95(json);
    let (bytes_m, _) = median_and_p95(bytes);
    let artwork = vec![0xABu8; 200_000];
    let t = Instant::now();
    let artwork_json = serde_json::to_string(&artwork).unwrap();
    let artwork_json_ms = ms(t);
    println!(
        "samples={samples} anlz_parse_ms={parse_m:.2}/{parse_p:.2} build_payload_ms={build_m:.2}/{build_p:.2} json_ms={json_m:.2}/{json_p:.2} payload_bytes={bytes_m:.0} artwork_200kb_json_bytes={} artwork_json_ms={artwork_json_ms:.2} (median/p95)",
        artwork_json.len()
    );
}

fn bands(paths: &[String]) {
    let mut ratios = Vec::new();
    for path in paths {
        let t = Instant::now();
        let waves = band_analysis::measure_waveforms(Path::new(path)).unwrap();
        let elapsed_ms = ms(t);
        let seconds = waves.pwv7.len() as f64 / 150.0;
        ratios.push(elapsed_ms / 1000.0 / seconds);
        println!(
            "{path}: {seconds:.0} s of audio, {elapsed_ms:.0} ms, {:.1}x realtime",
            seconds * 1000.0 / elapsed_ms
        );
    }
    let (median, _) = median_and_p95(ratios);
    println!("median {:.1} ms per second of audio", median * 1000.0);
}

fn save(dir: &Path, count: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let tracks: Vec<Track> = (0..count).map(fake_track).collect();
    let ids: Vec<u32> = lib.add_tracks(&tracks).unwrap().into_iter().map(|id| id as u32).collect();
    let step = (count / 500).max(1);
    let mut playlists: Vec<pioneer_usb_writer::models::Playlist> = (0..20)
        .map(|p| pioneer_usb_writer::models::Playlist {
            id: p as u32 + 1,
            name: format!("Playlist {p}"),
            track_ids: ids.iter().skip(p).step_by(step).copied().collect(),
            parent_id: 0,
            is_folder: false,
        })
        .collect();
    playlists.push(pioneer_usb_writer::models::Playlist {
        id: 21,
        name: "Big".into(),
        track_ids: ids.iter().step_by((count / 2000).max(1)).copied().collect(),
        parent_id: 0,
        is_folder: false,
    });
    let rows: usize = playlists.iter().map(|p| p.track_ids.len()).sum();

    let t = Instant::now();
    lib.replace_playlists(&playlists).unwrap();
    let first_ms = ms(t);

    let mut unchanged = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        lib.replace_playlists(&playlists).unwrap();
        unchanged.push(ms(t));
    }

    let mut rename = Vec::new();
    for n in 0..5 {
        playlists[0].name = format!("Renamed {n}");
        let t = Instant::now();
        lib.replace_playlists(&playlists).unwrap();
        rename.push(ms(t));
    }

    let mut append = Vec::new();
    for n in 0..5 {
        playlists[20].track_ids.push(ids[n * 5 + 1]);
        let t = Instant::now();
        lib.replace_playlists(&playlists).unwrap();
        append.push(ms(t));
    }

    let t = Instant::now();
    let loaded = lib.get_all_playlists().unwrap();
    let read_ms = ms(t);
    println!(
        "tracks={count} playlist_rows={rows} first_save_ms={first_ms:.1} unchanged_save_ms={:.1} rename_one_save_ms={:.1} append_one_save_ms={:.1} read_playlists_ms={read_ms:.1} playlists={}",
        median_and_p95(unchanged).0,
        median_and_p95(rename).0,
        median_and_p95(append).0,
        loaded.len(),
    );
}

fn tiny_mp3(n: usize) -> Vec<u8> {
    let text_frame = |id: &str, text: &str| {
        let mut body = vec![0u8];
        body.extend_from_slice(text.as_bytes());
        let mut frame = id.as_bytes().to_vec();
        frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&body);
        frame
    };
    let mut frames = text_frame("TIT2", &format!("Scan Track {n}"));
    frames.extend(text_frame("TPE1", &format!("Scan Artist {}", n % 200)));
    frames.extend(text_frame("TALB", &format!("Scan Album {}", n % 500)));
    let size = frames.len() as u32;
    let synchsafe = [(size >> 21) as u8 & 0x7f, (size >> 14) as u8 & 0x7f, (size >> 7) as u8 & 0x7f, size as u8 & 0x7f];
    let mut bytes = b"ID3\x03\x00\x00".to_vec();
    bytes.extend_from_slice(&synchsafe);
    bytes.extend(frames);
    for _ in 0..4 {
        bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        bytes.extend(std::iter::repeat(0u8).take(413));
    }
    bytes
}

fn scan(dir: &Path, count: usize) {
    let music = dir.join("music");
    std::fs::create_dir_all(&music).unwrap();
    for n in 0..count {
        let sub = music.join(format!("dir{}", n % 50));
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(format!("track{n:06}.mp3")), tiny_mp3(n)).unwrap();
    }

    let t = Instant::now();
    let scanned = pioneer_usb_writer::scanner::scan_directory(&music).unwrap();
    let scan_ms = ms(t);

    let per_track = { let _ = std::fs::remove_file(dir.join("per_track.db")); LocalLibrary::open(&dir.join("per_track.db")).unwrap() };
    let t = Instant::now();
    for track in &scanned {
        if !per_track.track_exists_by_path(&track.source_path).unwrap() {
            per_track.add_track(track).unwrap();
        }
    }
    let per_track_ms = ms(t);

    let batched = { let _ = std::fs::remove_file(dir.join("batched.db")); LocalLibrary::open(&dir.join("batched.db")).unwrap() };
    let t = Instant::now();
    let fresh: Vec<Track> = scanned
        .iter()
        .filter(|track| !batched.track_exists_by_path(&track.source_path).unwrap())
        .cloned()
        .collect();
    batched.add_tracks(&fresh).unwrap();
    let batched_ms = ms(t);

    println!(
        "files={count} scanned={} scan_directory_ms={scan_ms:.1} insert_per_track_ms={per_track_ms:.1} insert_batched_ms={batched_ms:.1}",
        scanned.len()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("gen") => generate(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("run") => run(Path::new(&args[2])),
        Some("load") => load(Path::new(&args[2]), args.get(3).map_or(60, |n| n.parse().unwrap())),
        Some("save") => save(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("scan") => scan(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("bands") => bands(&args[2..]),
        _ => eprintln!("usage: bench_startup gen <dir> <count> | run <dir> | load <dir> [samples] | save <dir> <tracks> | scan <dir> <files> | bands <audio files>"),
    }
}
