#[allow(dead_code)]
#[path = "../analysis_payload.rs"]
mod analysis_payload;
#[allow(dead_code)]
#[path = "../dto.rs"]
mod dto;
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;
#[allow(dead_code)]
#[path = "../track_infos.rs"]
mod track_infos;
#[allow(dead_code)]
#[path = "../import_plan.rs"]
mod import_plan;
#[allow(dead_code)]
#[path = "../app_log.rs"]
mod app_log;

#[path = "../tag_scan.rs"]
mod tag_scan;

#[path = "../command_error.rs"]
mod command_error;

#[path = "../waveform_detail.rs"]
mod waveform_detail;

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
    fake_analysis_of(i, TRACK_SECS)
}

fn fake_analysis_of(i: usize, track_secs: f64) -> AnalysisResult {
    let bpm = 90.0 + (i % 80) as f64;
    let beat_ms = (60_000.0 / bpm) as u32;
    let beats = (0..(track_secs * bpm / 60.0) as u32)
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
    let detail_len = (track_secs * 150.0) as usize;
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

fn perf(name: &str, value: serde_json::Value) {
    println!("PERF {}", serde_json::json!({ "bench": name, "values": value }));
}

fn run(dir: &Path) {
    let t = Instant::now();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let open_ms = ms(t);

    let t = Instant::now();
    let rows = lib.get_track_list_rows().unwrap();
    let rows_ms = ms(t);
    drop(rows);

    let t = Instant::now();
    let labels = lib.track_labels_reader().read_all();
    let labels_ms = ms(t);
    drop(labels);

    let reader = lib.track_labels_reader();
    let had_cache = dir.join(pioneer_library::LABELS_CACHE_FILE).exists();
    let t = Instant::now();
    drop(reader.read_cached());
    let labels_cached_ms = ms(t);

    let t = Instant::now();
    let changed = track_infos::changed_track_labels(&reader).len();
    let labels_refresh_ms = ms(t);

    let t = Instant::now();
    let with_previews = std::env::var_os("FF_BENCH_WITH_PREVIEWS").is_some();
    let infos = if with_previews {
        track_infos::build_track_infos(&lib).unwrap()
    } else {
        track_infos::list_track_infos(&lib).unwrap()
    };
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
    let table = (!with_previews).then(|| dto::TrackTable::new(&infos).unwrap());
    let table_ms = ms(t);
    let t = Instant::now();
    let table_json_len = table.as_ref().map_or(0, |table| serde_json::to_string(table).unwrap().len());
    let table_json_ms = ms(t);
    drop(table);
    let t = Instant::now();
    let json = if with_previews {
        serde_json::to_string(&serde_json::json!({ "tracks": infos, "playlists": playlists })).unwrap()
    } else {
        serde_json::to_string(&LoadedState { tracks: infos, playlists }).unwrap()
    };
    let json_ms = ms(t);

    println!(
        "tracks={count} open_ms={open_ms:.1} rows_ms={rows_ms:.1} labels_ms={labels_ms:.1} labels_cached_ms={labels_cached_ms:.1} (cache before: {had_cache}) labels_refresh_ms={labels_refresh_ms:.1} (changed {changed}) build_track_infos_ms={build_ms:.1} playlists_ms={playlists_ms:.1} json_ms={json_ms:.1} json_bytes={} load_state_total_ms={:.1}",
        json.len(),
        build_ms + playlists_ms + json_ms,
    );
    perf(
        "startup_backend",
        serde_json::json!({
            "tracks": count,
            "open_ms": open_ms,
            "track_rows_ms": rows_ms,
            "track_labels_ms": labels_ms,
            "track_labels_cached_ms": labels_cached_ms,
            "track_labels_refresh_ms": labels_refresh_ms,
            "build_track_infos_ms": build_ms,
            "playlists_ms": playlists_ms,
            "json_ms": json_ms,
            "table_build_ms": table_ms,
            "table_json_ms": table_json_ms,
            "table_json_bytes": table_json_len,
            "json_bytes": json.len(),
            "load_state_ms": build_ms + playlists_ms + json_ms,
        }),
    );
}

fn dump(dir: &Path, out: &Path) {
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let json = if std::env::var_os("FF_BENCH_WITH_PREVIEWS").is_some() {
        serde_json::to_vec(&serde_json::json!({ "tracks": track_infos::build_track_infos(&lib).unwrap() }))
    } else {
        serde_json::to_vec(&track_infos::loaded_state(&lib).unwrap())
    };
    std::fs::write(out, json.unwrap()).unwrap();
}

fn median_and_p95(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[(v.len() * 95 / 100).min(v.len() - 1)])
}

fn load(dir: &Path, samples: usize) {
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let unanalysed: std::collections::HashSet<i64> = lib.get_unanalyzed_track_ids().unwrap().into_iter().collect();
    let total = lib.track_count().unwrap() as usize;
    let analysed: Vec<i64> = (1..=total as i64).filter(|id| !unanalysed.contains(id)).collect();
    let step = (analysed.len() / samples).max(1);
    let picked: Vec<i64> = analysed.iter().step_by(step).take(samples).copied().collect();

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
    perf(
        "track_load_backend",
        serde_json::json!({
            "samples": samples,
            "anlz_parse_ms_median": parse_m, "anlz_parse_ms_p95": parse_p,
            "get_analysis_data_ms_median": build_m, "get_analysis_data_ms_p95": build_p,
            "json_ms_median": json_m, "json_ms_p95": json_p,
            "payload_bytes_median": bytes_m,
        }),
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

fn read_list(file: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(file).unwrap().lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect()
}

fn add_scanned_tracks(lib: &LocalLibrary, scanned: Vec<Track>) -> (usize, f64, f64) {
    let t = Instant::now();
    let mut seen = lib.track_paths().unwrap();
    let fresh: Vec<Track> = scanned.into_iter().filter(|t| seen.insert(t.source_path.clone())).collect();
    let exists_ms = ms(t);
    let t = Instant::now();
    let added = lib.add_tracks_covers_later(&fresh).unwrap().len();
    (added, exists_ms, ms(t))
}

fn import(folder: &Path, base_db: Option<&Path>, work: &Path) {
    std::fs::create_dir_all(work).unwrap();
    let db = work.join("import.db");
    let sidecars = |db: &Path| ["", "-wal", "-shm"].map(|ext| PathBuf::from(format!("{}{ext}", db.display())));
    let clear = || sidecars(&db).iter().for_each(|f| drop(std::fs::remove_file(f)));
    clear();
    if let Some(base) = base_db {
        for (from, to) in sidecars(base).iter().zip(sidecars(&db).iter()) {
            if from.exists() {
                std::fs::copy(from, to).unwrap();
            }
        }
    }
    let lib = LocalLibrary::open(&db).unwrap();
    let library_tracks = lib.track_count().unwrap();
    let paths = vec![folder.to_string_lossy().into_owned()];

    let total = Instant::now();
    let t = Instant::now();
    let known = lib.track_paths().unwrap();
    let known_ms = ms(t);
    let t = Instant::now();
    let survey = import_plan::survey_files(&paths);
    let survey_ms = ms(t);
    let t = Instant::now();
    let scanned = import_plan::scan_paths(&survey, &known, &mut || {});
    let scan_ms = ms(t);
    let files = scanned.tracks.len() + scanned.known.len();
    let plans = import_plan::ImportPlans::default();
    let t = Instant::now();
    let plan = import_plan::plan_for_library(&plans, &lib, scanned, &survey).unwrap();
    if let Ok(out) = std::env::var("FF_IMPORT_PLAN_OUT") {
        std::fs::write(out, serde_json::to_string_pretty(&plan).unwrap()).unwrap();
    }
    let plan_ms = ms(t);
    let plan_import_ms = ms(total);
    let t = Instant::now();
    let (tracks, _) = plans.take(plan.token).unwrap().resolve(false);
    let (added, exists_ms, insert_ms) = add_scanned_tracks(&lib, tracks);
    let commit_ms = ms(t);
    if let Ok(again) = std::env::var("FF_IMPORT_AGAIN") {
        let t = Instant::now();
        let known = lib.track_paths().unwrap();
        let survey = import_plan::survey_files(&[again]);
        let scanned = import_plan::scan_paths(&survey, &known, &mut || {});
        let second_scan_ms = ms(t);
        let t = Instant::now();
        let plan = import_plan::plan_for_library(&plans, &lib, scanned, &survey).unwrap();
        let second_plan_ms = ms(t);
        println!("second_import: scan_ms={second_scan_ms:.1} plan_ms={second_plan_ms:.1} duplicates={}", plan.duplicates.len());
        perf(
            "import_second",
            serde_json::json!({ "scan_ms": second_scan_ms, "plan_ms": second_plan_ms, "duplicates": plan.duplicates.len() }),
        );
    }
    let lib = std::sync::Mutex::new(lib);
    let t = Instant::now();
    let covers = import_plan::store_pending_covers(&lib, None, &mut |_| {}).unwrap();
    let covers_ms = ms(t);
    let lib = lib.into_inner().unwrap();
    println!(
        "files={files} library_tracks={library_tracks} known_ms={known_ms:.1} survey_ms={survey_ms:.1} scan_ms={scan_ms:.1} plan_ms={plan_ms:.1} plan_import_ms={plan_import_ms:.1} commit_import_ms={commit_ms:.1} (exists_check_ms={exists_ms:.1} add_tracks_ms={insert_ms:.1}) rows_ms={:.1} covers_ms={covers_ms:.1} covers={covers} added={added} duplicates={}",
        plan_import_ms + commit_ms,
        plan.duplicates.len()
    );
    perf(
        "import_backend",
        serde_json::json!({
            "files": files,
            "library_tracks": library_tracks,
            "survey_ms": survey_ms,
            "scan_ms": scan_ms,
            "plan_ms": plan_ms,
            "plan_import_ms": plan_import_ms,
            "commit_import_ms": commit_ms,
            "commit_exists_check_ms": exists_ms,
            "commit_add_tracks_ms": insert_ms,
            "known_paths_ms": known_ms,
            "covers_ms": covers_ms,
            "covers": covers,
            "added": added,
            "duplicates": plan.duplicates.len(),
        }),
    );
    drop(lib);
    if let Ok(keep) = std::env::var("FF_IMPORT_KEEP_DB") {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let checkpointed = LocalLibrary::open(&db).unwrap();
        drop(checkpointed);
        std::fs::copy(&db, keep).unwrap();
    }
    clear();
}

fn decode_start(path: &Path) -> (f64, f64, f64) {
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use symphonia::core::units::Time;

    let t = Instant::now();
    let file = std::fs::File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).unwrap().format;
    let track = format.default_track().unwrap().clone();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).unwrap();
    let probe_ms = ms(t);
    let mut first_packet = || loop {
        let packet = format.next_packet().unwrap();
        if packet.track_id() == track.id && decoder.decode(&packet).is_ok() {
            return;
        }
    };
    first_packet();
    let first_ms = ms(t);
    let secs = track.codec_params.n_frames.zip(track.codec_params.sample_rate).map_or(60.0, |(n, r)| n as f64 / r as f64);
    let t = Instant::now();
    let mut decode_after_seek = || {
        format.seek(SeekMode::Accurate, SeekTo::Time { time: Time::from(secs * 0.5), track_id: Some(track.id) }).unwrap();
        decoder.reset();
        loop {
            let packet = format.next_packet().unwrap();
            if packet.track_id() == track.id && decoder.decode(&packet).is_ok() {
                return;
            }
        }
    };
    decode_after_seek();
    (probe_ms, first_ms, ms(t))
}

fn track_audio(list: &Path, work: &Path) {
    let files = read_list(list);
    let mut probe = Vec::new();
    let mut first = Vec::new();
    let mut seek = Vec::new();
    for _ in 0..2 {
        for f in &files {
            let (p, a, s) = decode_start(f);
            probe.push(p);
            first.push(a);
            seek.push(s);
        }
    }
    let half = files.len();
    let (probe_m, probe_p) = median_and_p95(probe[half..].to_vec());
    let (first_m, first_p) = median_and_p95(first[half..].to_vec());
    let (seek_m, seek_p) = median_and_p95(seek[half..].to_vec());

    let cache = work.join("waveform-detail");
    let _ = std::fs::remove_dir_all(&cache);
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut bytes = Vec::new();
    for f in &files {
        let t = Instant::now();
        let b = waveform_detail::detail_bytes(&cache, f).unwrap();
        cold.push(ms(t));
        bytes.push(b.len() as f64);
        let t = Instant::now();
        waveform_detail::detail_bytes(&cache, f).unwrap();
        warm.push(ms(t));
    }
    let _ = std::fs::remove_dir_all(&cache);
    let (cold_m, cold_p) = median_and_p95(cold);
    let (warm_m, warm_p) = median_and_p95(warm);
    let (bytes_m, _) = median_and_p95(bytes);
    println!(
        "files={} decode_probe_ms={probe_m:.1}/{probe_p:.1} decode_first_packet_ms={first_m:.1}/{first_p:.1} seek_mid_decode_ms={seek_m:.1}/{seek_p:.1} detail_cold_ms={cold_m:.0}/{cold_p:.0} detail_cached_ms={warm_m:.1}/{warm_p:.1} detail_bytes={bytes_m:.0} (median/p95)",
        files.len()
    );
    perf(
        "track_audio_backend",
        serde_json::json!({
            "files": files.len(),
            "decode_probe_ms_median": probe_m, "decode_probe_ms_p95": probe_p,
            "decode_first_packet_ms_median": first_m, "decode_first_packet_ms_p95": first_p,
            "seek_mid_decode_ms_median": seek_m, "seek_mid_decode_ms_p95": seek_p,
            "waveform_detail_cold_ms_median": cold_m, "waveform_detail_cold_ms_p95": cold_p,
            "waveform_detail_cached_ms_median": warm_m, "waveform_detail_cached_ms_p95": warm_p,
            "waveform_detail_bytes_median": bytes_m,
        }),
    );
}

/// Per file, back to back: the single-threaded decode and the meter on their own, the one-thread detail measurement
/// (decode then meter, as before Q50) against `measure_detail`, and whether the parallel decode and the detail bytes are
/// bit-identical to the single-threaded ones.
fn detail_profile(list: &Path) {
    use pioneer_usb_writer::waveform::DetailMeter;
    let serial_detail = |f: &Path| {
        let mut meter: Option<DetailMeter> = None;
        band_analysis::decode_blocks(f, |rate, channels, samples| {
            meter.get_or_insert_with(|| DetailMeter::new(rate, channels)).push(samples);
        })
        .unwrap();
        meter.unwrap().finish().to_bytes()
    };
    let files = read_list(list);
    let (mut decode, mut meter, mut serial, mut parallel) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut mismatches = Vec::new();
    for f in &files {
        let t = Instant::now();
        let mut audio: Vec<f32> = Vec::new();
        let mut spec = (0u32, 0usize);
        if let Err(e) = band_analysis::decode_blocks(f, |rate, channels, samples| {
            spec = (rate, channels);
            audio.extend_from_slice(samples);
        }) {
            eprintln!("skip {}: {e}", f.display());
            continue;
        }
        decode.push(ms(t));
        let t = Instant::now();
        let mut m = DetailMeter::new(spec.0, spec.1);
        m.push(&audio);
        std::hint::black_box(m.finish());
        meter.push(ms(t));

        let mut threaded: Vec<f32> = Vec::with_capacity(audio.len());
        band_analysis::decode_blocks_parallel(f, band_analysis::decode_threads(), |_, _, samples| {
            threaded.extend_from_slice(samples)
        })
        .unwrap();
        let same_audio = threaded.len() == audio.len() && threaded.iter().zip(&audio).all(|(a, b)| a.to_bits() == b.to_bits());

        let t = Instant::now();
        let old = serial_detail(f);
        serial.push(ms(t));
        let t = Instant::now();
        let new = band_analysis::measure_detail(f).unwrap().to_bytes();
        parallel.push(ms(t));
        if !same_audio || old != new {
            mismatches.push(format!("{} (audio {} vs {} samples, same {same_audio})", f.display(), threaded.len(), audio.len()));
        }
    }
    let measured = decode.len();
    let (d, _) = median_and_p95(decode);
    let (m, _) = median_and_p95(meter);
    let (s, sp) = median_and_p95(serial);
    let (p, pp) = median_and_p95(parallel);
    for m in &mismatches {
        println!("MISMATCH {m}");
    }
    println!(
        "files={} threads={} decode_ms={d:.0} meter_ms={m:.0} detail_one_thread_ms={s:.0}/{sp:.0} measure_detail_ms={p:.0}/{pp:.0} identical={}/{} (median/p95)",
        measured,
        band_analysis::decode_threads(),
        measured - mismatches.len(),
        measured
    );
    perf(
        "waveform_detail_profile",
        serde_json::json!({
            "files": measured, "threads": band_analysis::decode_threads(),
            "decode_ms_median": d, "meter_ms_median": m,
            "detail_one_thread_ms_median": s, "detail_one_thread_ms_p95": sp,
            "measure_detail_ms_median": p, "measure_detail_ms_p95": pp,
            "identical": measured - mismatches.len(),
        }),
    );
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn fixture_lib(music: &Path, dir: &Path, analysed: f64, seed: u64) {
    use pioneer_library::rekordbox::{ImportedMeta, SourceKind};
    std::fs::create_dir_all(dir).unwrap();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let mut rng = Rng(seed.max(1));
    let total = Instant::now();

    let t = Instant::now();
    let mut scanned = import_plan::scan_paths(&import_plan::survey_files(&[music.to_string_lossy().into_owned()]), &Default::default(), &mut || {}).tracks;
    scanned.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    let scan_ms = ms(t);

    let t = Instant::now();
    let ids = lib.add_tracks_covers_later(&scanned).unwrap();
    let lib = std::sync::Mutex::new(lib);
    let with_art = import_plan::store_pending_covers(&lib, None, &mut |_| {}).unwrap();
    let lib = lib.into_inner().unwrap();
    let add_ms = ms(t);

    let t = Instant::now();
    let mut analysed_ids = Vec::new();
    let mut batch: Vec<(i64, AnalysisResult)> = Vec::new();
    let flush = |batch: &mut Vec<(i64, AnalysisResult)>| {
        lib.set_analyses(&batch.iter().map(|(id, a)| (*id, a)).collect::<Vec<_>>()).unwrap();
        batch.clear();
    };
    for (n, id) in ids.iter().enumerate() {
        if rng.unit() >= analysed {
            continue;
        }
        let nominal_secs = 180.0 + rng.below(300) as f64;
        batch.push((*id, fake_analysis_of(n, nominal_secs)));
        analysed_ids.push(*id);
        if batch.len() == 500 {
            flush(&mut batch);
            eprintln!("[fixture] analysed {}/{}", analysed_ids.len(), ids.len());
        }
    }
    flush(&mut batch);
    let analysis_ms = ms(t);

    let t = Instant::now();
    let mut rated = 0;
    let mut coloured = 0;
    let colours = ["#ff0000", "#ff8c00", "#ffd700", "#00c853", "#00b0ff", "#2962ff", "#aa00ff", "#ff4081"];
    for id in &ids {
        if rng.unit() < 0.25 {
            lib.set_user_rating(*id, 1 + rng.below(5) as u8).unwrap();
            rated += 1;
        }
        if rng.unit() < 0.08 {
            let meta = ImportedMeta {
                source: SourceKind::Xml,
                source_id: id.to_string(),
                grid_locked: false,
                cues_locked: false,
                rating: None,
                color: Some(colours[rng.below(colours.len())].to_string()),
                cues: Vec::new(),
                grid: Vec::new(),
            };
            meta.write(&dir.join("anlz").join(id.to_string())).unwrap();
            coloured += 1;
        }
    }
    let labels_ms = ms(t);

    let t = Instant::now();
    let mut playlists = 0;
    let mut folders = 0;
    let mut playlist_rows = 0;
    let sizes = [12, 25, 40, 60, 80, 120, 150, 250, 400, 800];
    let tops = ["Gigs", "Genres", "Crates", "2024", "2025", "Archive", "Radio Shows"];
    for (f, top) in tops.iter().enumerate() {
        let top_id = lib.create_folder(top, 0).unwrap() as u32;
        folders += 1;
        let subs: Vec<u32> = (0..rng.below(3))
            .map(|s| {
                folders += 1;
                lib.create_folder(&format!("{top} {}", s + 1), top_id).unwrap() as u32
            })
            .collect();
        for p in 0..(6 + rng.below(8)) {
            let parent = if subs.is_empty() || rng.unit() < 0.5 { top_id } else { subs[rng.below(subs.len())] };
            let pid = lib.create_playlist_in(&format!("{top} set {}", f * 20 + p + 1), parent).unwrap();
            let len = sizes[rng.below(sizes.len())].min(ids.len());
            let start = rng.below(ids.len());
            let members: Vec<i64> = (0..len).map(|k| ids[(start + k * 7) % ids.len()]).collect();
            playlist_rows += members.len();
            lib.set_playlist_tracks(pid, &members).unwrap();
            playlists += 1;
        }
    }
    let big = lib.create_playlist("All Bangers").unwrap();
    let members: Vec<i64> = ids.iter().step_by(5).copied().collect();
    playlist_rows += members.len();
    lib.set_playlist_tracks(big, &members).unwrap();
    playlists += 1;
    let playlists_ms = ms(t);

    perf(
        "fixture_library",
        serde_json::json!({
            "tracks": ids.len(), "with_artwork": with_art, "analysed": analysed_ids.len(),
            "rated": rated, "coloured": coloured, "playlists": playlists, "folders": folders, "playlist_rows": playlist_rows,
            "scan_ms": scan_ms, "add_tracks_ms": add_ms, "analysis_ms": analysis_ms, "labels_ms": labels_ms,
            "playlists_ms": playlists_ms, "total_ms": ms(total),
        }),
    );
}

fn rss_mb() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    kb / 1024.0
}

fn fake_detail_series(seed: usize) -> pioneer_usb_writer::waveform::DetailSeries {
    let columns = 2400 * 60;
    pioneer_usb_writer::waveform::DetailSeries {
        columns_per_second: 2400,
        gains: [1, 2, 3],
        signal_gain: 1.0,
        planes: [
            (0..columns).map(|n| ((n + seed) % 120) as u8).collect(),
            (0..columns).map(|n| ((n * 3 + seed) % 100) as u8).collect(),
            (0..columns).map(|n| ((n * 5) % 90) as u8).collect(),
        ],
        signal: [vec![-20; columns], vec![20; columns]],
    }
}

/// Library reads, analysis stores, playlist saves and detail-cache writes in a loop on a copy of the library, RSS sampled over time.
fn soak(dir: &Path, seconds: u64) {
    let work = dir.join(format!("soak-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::copy(dir.join("library.db"), work.join("library.db")).unwrap();
    let lib = LocalLibrary::open(&work.join("library.db")).unwrap();
    let total = lib.track_count().unwrap() as usize;
    let audio_files: Vec<PathBuf> = (0..40)
        .map(|n| {
            let path = work.join(format!("track{n}.wav"));
            std::fs::write(&path, [n as u8]).unwrap();
            path
        })
        .collect();
    let cache = work.join("cache");
    let reader = lib.track_labels_reader();
    let mut playlists = lib.get_all_playlists().unwrap();
    let parts = std::env::var("FF_SOAK_PARTS").unwrap_or_else(|_| "state,analysis,detail,playlists".to_string());
    let part = |name: &str| parts.split(',').any(|p| p == name);
    let started = Instant::now();
    let mut rounds = 0usize;
    let mut samples: Vec<(f64, usize, f64)> = Vec::new();
    let mut next_sample = 0.0;
    while started.elapsed().as_secs() < seconds {
        if part("state") {
            let infos = track_infos::list_track_infos(&lib).unwrap();
            let state = LoadedState {
                tracks: infos,
                playlists: playlists
                    .iter()
                    .map(|pl| PlaylistInput { id: pl.id, name: pl.name.clone(), track_ids: pl.track_ids.clone(), parent_id: pl.parent_id, is_folder: pl.is_folder })
                    .collect(),
            };
            drop(serde_json::to_string(&state).unwrap());
            drop(state);
            drop(track_infos::changed_track_labels(&reader));
        }
        if part("analysis") {
            for n in 0..20 {
                let id = ((rounds * 20 + n) * 7919 % total) as i64 + 1;
                drop(analysis_payload::build_analysis_json(&lib, id).unwrap());
                lib.set_analysis(id, &fake_analysis(rounds + n)).unwrap();
            }
        }
        if part("detail") {
            for (n, audio) in audio_files.iter().enumerate() {
                waveform_detail::store_detail(&cache, audio, &fake_detail_series(rounds + n)).unwrap();
                drop(waveform_detail::detail_bytes(&cache, audio).unwrap());
            }
        }
        if part("playlists") {
            playlists[0].name = format!("Soak {}", rounds % 7);
            lib.replace_playlists(&playlists).unwrap();
        }
        rounds += 1;
        if started.elapsed().as_secs_f64() >= next_sample {
            let sample = (started.elapsed().as_secs_f64(), rounds, rss_mb());
            println!("soak_backend t={:.0}s rounds={} rss_mb={:.1}", sample.0, sample.1, sample.2);
            samples.push(sample);
            next_sample += (seconds as f64 / 12.0).max(5.0);
        }
    }
    let end = (started.elapsed().as_secs_f64(), rounds, rss_mb());
    println!("soak_backend t={:.0}s rounds={} rss_mb={:.1} (end)", end.0, end.1, end.2);
    std::fs::remove_dir_all(&work).ok();
    perf(
        "soak_backend",
        serde_json::json!({
            "tracks": total,
            "rounds": end.1,
            "rss_mb_first": samples.first().map_or(0.0, |s| s.2),
            "rss_mb_last": end.2,
            "samples": samples.iter().map(|s| serde_json::json!({ "t": s.0, "rounds": s.1, "rss_mb": s.2 })).collect::<Vec<_>>(),
        }),
    );
}

fn rep(dir: &Path, runs: usize) {
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let mut columns: [Vec<f64>; 4] = Default::default();
    let mut bytes = 0;
    for _ in 0..runs {
        let t = Instant::now();
        drop(lib.get_track_list_rows_without_previews().unwrap());
        columns[0].push(ms(t));
        let t = Instant::now();
        let state = track_infos::loaded_state(&lib).unwrap();
        columns[1].push(ms(t));
        let t = Instant::now();
        drop(dto::TrackTable::new(&state.tracks).unwrap());
        columns[2].push(ms(t));
        let t = Instant::now();
        bytes = serde_json::to_vec(&state).unwrap().len();
        columns[3].push(ms(t));
    }
    let total: Vec<f64> = columns[1].iter().zip(&columns[3]).map(|(a, b)| a + b).collect();
    let summary = |name: &str, v: &[f64]| {
        let mut sorted = v.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        format!("{name}={:.1} ({:.1}-{:.1})", sorted[sorted.len() / 2], sorted[0], sorted[sorted.len() - 1])
    };
    println!(
        "REP runs={runs} bytes={bytes} {} {} {} {} {}",
        summary("rows_ms", &columns[0]),
        summary("loaded_state_ms", &columns[1]),
        summary("table_build_ms", &columns[2]),
        summary("serialize_ms", &columns[3]),
        summary("load_state_ms", &total),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("gen") => generate(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("run") => run(Path::new(&args[2])),
        Some("rep") => rep(Path::new(&args[2]), args.get(3).map_or(15, |n| n.parse().unwrap())),
        Some("dump") => dump(Path::new(&args[2]), Path::new(&args[3])),
        Some("load") => load(Path::new(&args[2]), args.get(3).map_or(60, |n| n.parse().unwrap())),
        Some("save") => save(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("scan") => scan(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("bands") => bands(&args[2..]),
        Some("import") => import(Path::new(&args[2]), args.get(4).map(Path::new), Path::new(&args[3])),
        Some("soak") => soak(Path::new(&args[2]), args.get(3).map_or(60, |n| n.parse().unwrap())),
        Some("detail") => detail_profile(Path::new(&args[2])),
        Some("audio") => track_audio(Path::new(&args[2]), Path::new(&args[3])),
        Some("fixture-lib") => fixture_lib(
            Path::new(&args[2]),
            Path::new(&args[3]),
            args.get(4).map_or(0.7, |a| a.parse().unwrap()),
            args.get(5).map_or(4404, |s| s.parse().unwrap()),
        ),
        _ => eprintln!(
            "usage: bench_startup gen <dir> <count> | run <dir> | rep <dir> [runs] | dump <dir> <out.json> | load <dir> [samples] | save <dir> <tracks> | scan <dir> <files> | bands <audio files> | import <folder> <workdir> [base library.db] | audio <file list> <workdir> | soak <dir> [seconds] | fixture-lib <music dir> <library dir> [analysed share] [seed]"
        ),
    }
}
