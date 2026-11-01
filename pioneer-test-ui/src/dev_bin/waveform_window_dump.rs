//! Fine zoom windows outside the app, for latency numbers and the zoom screenshots (`app/perf/zoom-detail-shots.mjs`).
//!
//! `waveform_window_dump bench <audio>...` prints how long one window takes at three zoom levels (seek, decode of the
//! window plus its pre-roll, measuring).
//! `waveform_window_dump serve <port> <audio>` answers `GET /window?start=S&end=S&columns=N` with the `FFWW` bytes the
//! `get_waveform_window` command returns.
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::Instant;

const SCREEN_COLUMNS: usize = 2256;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bench") => args.for_each(|audio| bench(Path::new(&audio))),
        Some("serve") => {
            let port = args.next().expect("port");
            let audio = args.next().expect("audio");
            serve(&port, Path::new(&audio));
        }
        _ => panic!("usage: waveform_window_dump bench <audio>... | serve <port> <audio>"),
    }
}

fn bench(audio: &Path) {
    let detail = band_analysis::measure_detail(audio).expect("measure detail");
    let duration_s = detail.len() as f64 / detail.columns_per_second as f64;
    let name = audio.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
    println!("{name}: {duration_s:.1} s");
    for screen_s in [0.9, 0.1, 0.004] {
        let mut times: Vec<f64> = (0..24)
            .map(|i| {
                let centre = duration_s * (0.05 + 0.9 * ((i * 7919) % 24) as f64 / 24.0);
                let (start, end) = (centre - 1.5 * screen_s, centre + 1.5 * screen_s);
                let started = Instant::now();
                band_analysis::measure_detail_window(audio, detail.gains, start, end, 3 * SCREEN_COLUMNS, &|| true)
                    .expect("window")
                    .expect("not superseded");
                started.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        println!(
            "  {:>6.1} ms per screen: window p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
            screen_s * 1000.0,
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times[times.len() - 1],
        );
    }
}

fn serve(port: &str, audio: &Path) {
    let detail = band_analysis::measure_detail(audio).expect("measure detail");
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).expect("bind");
    println!("serving windows of {} on {port}", audio.display());
    for stream in listener.incoming().flatten() {
        let mut stream = stream;
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            continue;
        }
        let query = line.split_whitespace().nth(1).and_then(|p| p.split_once('?')).map_or("", |(_, q)| q).to_string();
        let param = |key: &str| -> f64 {
            query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == key)
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0.0)
        };
        let started = Instant::now();
        let window = band_analysis::measure_detail_window(
            audio,
            detail.gains,
            param("start"),
            param("end"),
            param("columns") as usize,
            &|| true,
        );
        let body = match window {
            Ok(Some(window)) => window.to_bytes(),
            _ => Vec::new(),
        };
        println!("window {:.4}..{:.4} s: {:.1} ms", param("start"), param("end"), started.elapsed().as_secs_f64() * 1000.0);
        let status = if body.is_empty() { "500 Internal Server Error" } else { "200 OK" };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).ok();
        stream.write_all(&body).ok();
    }
}
