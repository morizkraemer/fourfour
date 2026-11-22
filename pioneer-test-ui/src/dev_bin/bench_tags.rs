#[allow(dead_code)]
#[path = "../tag_scan.rs"]
mod tag_scan;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use pioneer_usb_writer::scanner::read_track_metadata;

fn disk_read_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|io| io.lines().find_map(|l| l.strip_prefix("read_bytes: ").and_then(|v| v.trim().parse().ok())))
        .unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, mode, list] = args.as_slice() else {
        eprintln!("usage: bench_tags tags|full <file list>");
        std::process::exit(2);
    };
    let paths: Vec<String> = std::fs::read_to_string(list).unwrap().lines().map(str::to_string).collect();
    let mut per_ext: BTreeMap<String, (usize, u64, f64)> = BTreeMap::new();
    for path in &paths {
        let path = Path::new(path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let before = disk_read_bytes();
        let started = Instant::now();
        let ok = match mode.as_str() {
            "tags" => tag_scan::read_track_tags(path, 0).is_ok(),
            _ => read_track_metadata(path, 0).is_ok(),
        };
        let entry = per_ext.entry(ext).or_default();
        entry.0 += usize::from(ok);
        entry.1 += disk_read_bytes() - before;
        entry.2 += started.elapsed().as_secs_f64();
    }
    let mut total = 0;
    for (ext, (files, bytes, secs)) in &per_ext {
        total += bytes;
        println!("{ext}: files={files} read_mb={:.1} avg_kb={:.1} secs={secs:.2}", *bytes as f64 / 1e6, *bytes as f64 / 1e3 / *files as f64);
    }
    println!("total read_mb={:.1}", total as f64 / 1e6);
}
