//! Measures syncing 1, 2 and 3 sticks serially vs concurrently.
//!
//! `cargo bench -p pioneer-library --bench multi_sync_bench`
//! Env: BENCH_DIR (default: system temp dir), BENCH_TRACKS (default 500),
//! BENCH_SECONDS (audio seconds per track, default 3),
//! BENCH_WRITER=files (copy + ANLZ only; the full writer panics above ~200 tracks).
//! Reads /proc, so Linux only.

#[path = "../tests/support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use pioneer_library::{
    LocalLibrary, NoProgress, StickStatus, SyncSelection, SyncTarget, statvfs_available_bytes,
};
use pioneer_usb_writer::models::{AnalysisResult, Playlist, SyncReport, Track};
use pioneer_usb_writer::writer::{self, anlz};

fn files_only_writer(
    root: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    _playlists: &[Playlist],
) -> Result<SyncReport> {
    for (track, analysis) in tracks.iter().zip(analyses) {
        let dest = root.join(track.usb_path.trim_start_matches('/'));
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::copy(&track.source_path, &dest)?;
        anlz::write_anlz_dat(&root.join(anlz::anlz_path_for_track(track)), track, analysis)?;
        anlz::write_anlz_ext(&root.join(anlz::anlz_ext_path_for_track(track)), track, analysis)?;
    }
    Ok(SyncReport {
        tracks_added: tracks.len() as u32,
        tracks_updated: 0,
        tracks_replaced: 0,
        tracks_removed: 0,
        tracks_unchanged: 0,
    })
}

#[derive(Clone, Copy)]
struct Usage {
    cpu_secs: f64,
    read_bytes: u64,
    write_bytes: u64,
    wchar: u64,
}

fn usage() -> Usage {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let after_comm = stat.rsplit_once(')').unwrap().1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let ticks = |i: usize| fields[i].parse::<f64>().unwrap();
    let cpu_secs = (ticks(11) + ticks(12)) / 100.0;
    let io = std::fs::read_to_string("/proc/self/io").unwrap();
    let field = |name: &str| -> u64 {
        io.lines()
            .find_map(|l| l.strip_prefix(name))
            .map(|v| v.trim().parse().unwrap())
            .unwrap()
    };
    Usage {
        cpu_secs,
        read_bytes: field("read_bytes:"),
        write_bytes: field("write_bytes:"),
        wchar: field("wchar:"),
    }
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn fresh_sticks(base: &Path, label: &str, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|i| {
            let dir = base.join(format!("{label}-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            dir
        })
        .collect()
}

fn targets(sticks: &[PathBuf]) -> Vec<SyncTarget> {
    sticks
        .iter()
        .map(|s| SyncTarget { volume: s.clone(), selection: SyncSelection::WholeLibrary })
        .collect()
}

struct Measured {
    wall: f64,
    cpu: f64,
    usage_delta: (u64, u64, u64),
}

fn measure(work: impl FnOnce()) -> Measured {
    let before = usage();
    let started = Instant::now();
    work();
    let wall = started.elapsed().as_secs_f64();
    let after = usage();
    Measured {
        wall,
        cpu: after.cpu_secs - before.cpu_secs,
        usage_delta: (
            after.read_bytes - before.read_bytes,
            after.write_bytes - before.write_bytes,
            after.wchar - before.wchar,
        ),
    }
}

fn run_all(library: &LocalLibrary, sticks: &[PathBuf]) {
    let write: pioneer_library::WriterFn = match std::env::var("BENCH_WRITER").as_deref() {
        Ok("files") => files_only_writer,
        _ => writer::filesystem::sync_usb,
    };
    let reports = library
        .prepare_multi_sync(&targets(sticks))
        .unwrap()
        .run_with(&NoProgress, &statvfs_available_bytes, write);
    for report in reports {
        assert_eq!(report.status, StickStatus::Done, "{:?}", report.error);
    }
}

fn dir_bytes(dir: &Path) -> u64 {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

fn main() {
    let tracks: usize = env_or("BENCH_TRACKS", 500);
    let seconds: f32 = env_or("BENCH_SECONDS", 3.0);
    let base_dir = std::env::var("BENCH_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let work = base_dir.join(format!("fourfour-multisync-bench-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();

    println!("dir={} tracks={tracks} seconds/track={seconds}", work.display());
    let library = LocalLibrary::open_in_memory().unwrap();
    let setup = Instant::now();
    support::populate_library(&library, &work.join("music"), tracks, seconds);
    let audio_bytes = dir_bytes(&work.join("music"));
    println!("library setup {:.2}s, audio {:.1} MB", setup.elapsed().as_secs_f64(), audio_bytes as f64 / 1e6);

    let read_once = measure(|| {
        library.prepare_multi_sync(&targets(&fresh_sticks(&work, "prep", 3))).unwrap();
    });
    println!("prepare (read+prepare once, 3 targets): {:.3}s wall", read_once.wall);

    let copy_only = measure(|| {
        let sticks = fresh_sticks(&work, "copyonly", 1);
        let (library_tracks, _) = library.get_analyzed_tracks().unwrap();
        for t in &library_tracks {
            let dest = sticks[0].join(t.usb_path.trim_start_matches('/'));
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(&t.source_path, dest).unwrap();
        }
    });
    println!("baseline: plain copy of audio to 1 dir: {:.3}s wall, {:.3}s cpu", copy_only.wall, copy_only.cpu);

    println!("\nsticks | mode       | wall s | cpu s | cpu/wall | read MB | write MB (kernel) | wchar MB");
    for k in 1..=3usize {
        let serial = measure(|| {
            for (i, stick) in fresh_sticks(&work, &format!("serial{k}"), k).iter().enumerate() {
                let _ = i;
                run_all(&library, std::slice::from_ref(stick));
            }
        });
        let concurrent = measure(|| run_all(&library, &fresh_sticks(&work, &format!("conc{k}"), k)));
        for (mode, m) in [("serial", &serial), ("concurrent", &concurrent)] {
            println!(
                "{k:6} | {mode:10} | {:6.3} | {:5.2} | {:8.2} | {:7.1} | {:17.1} | {:8.1}",
                m.wall,
                m.cpu,
                m.cpu / m.wall,
                m.usage_delta.0 as f64 / 1e6,
                m.usage_delta.1 as f64 / 1e6,
                m.usage_delta.2 as f64 / 1e6,
            );
        }
        println!("       speedup {:.2}x", serial.wall / concurrent.wall);
    }

    let resync = measure(|| run_all(&library, &fresh_sticks(&work, "conc3", 3)));
    println!("\nresync of 3 unchanged sticks (concurrent): {:.3}s wall", resync.wall);

    std::fs::remove_dir_all(&work).unwrap();
}
