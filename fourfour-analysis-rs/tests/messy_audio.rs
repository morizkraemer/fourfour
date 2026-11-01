#[allow(dead_code)]
mod messy_fixtures;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, Analyzer, PoolOptions};
use messy_fixtures::*;

const DECODE_BUDGET: Duration = Duration::from_secs(20);

fn guarded<T>(label: &str, work: impl FnOnce() -> T) -> Result<T, String> {
    let started = Instant::now();
    let result = catch_unwind(AssertUnwindSafe(work)).map_err(|p| {
        let message = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        format!("PANIC in {label}: {message}")
    });
    if started.elapsed() > DECODE_BUDGET {
        return Err(format!("SLOW {label}: {:.1}s", started.elapsed().as_secs_f64()));
    }
    result
}

#[test]
fn damaged_audio_never_panics_or_hangs_the_decoder() {
    let scratch = Scratch::new("decode");
    let encoded = Encoded::make(scratch.path());
    let files = damaged_files(scratch.path(), &encoded);
    let mut failures = Vec::new();
    for file in &files {
        match guarded(&file.label, || decode_mono(&file.path)) {
            Ok(Ok(mono)) => {
                eprintln!("DECODE {:<34} ok {} samples truncated={}", file.label, mono.samples.len(), mono.truncated);
                if mono.samples.is_empty() || mono.rate == 0 {
                    failures.push(format!("{}: Ok with {} samples at {} Hz", file.label, mono.samples.len(), mono.rate));
                }
                if mono.gapless.end > mono.samples.len() || mono.gapless.start > mono.gapless.end {
                    failures.push(format!("{}: gapless range {:?} outside {} samples", file.label, mono.gapless, mono.samples.len()));
                }
            }
            Ok(Err(e)) => eprintln!("DECODE {:<34} err {e}", file.label),
            Err(problem) => failures.push(problem),
        }
    }
    assert!(failures.is_empty(), "{} of {} files:\n{}", failures.len(), files.len(), failures.join("\n"));
}

#[test]
fn damaged_mp3_headers_do_not_panic_the_frame_scan() {
    let scratch = Scratch::new("mp3scan");
    let encoded = Encoded::make(scratch.path());
    let mp3 = encoded.mp3.clone().unwrap_or_else(synthetic_mp3);
    let mut failures = Vec::new();
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for keep in [0usize, 1, 3, 4, 5, 9, 10, 11, 100, 417, 418, 700, 1200, 4000, 4100, 17000] {
        cases.push((format!("mp3 cut at {keep} bytes"), mp3[..keep.min(mp3.len())].to_vec()));
    }
    for (name, tag_size) in [("id3 claims 256 MB", 0x0fff_ffffusize), ("id3 claims 1 GB", 0x0fff_fff0), ("id3 claims exact file", mp3.len())] {
        let mut bytes = b"ID3".to_vec();
        bytes.extend_from_slice(&[4, 0, 0]);
        bytes.extend_from_slice(&[((tag_size >> 21) & 0x7f) as u8, ((tag_size >> 14) & 0x7f) as u8, ((tag_size >> 7) & 0x7f) as u8, (tag_size & 0x7f) as u8]);
        bytes.extend_from_slice(&mp3);
        cases.push((name.into(), bytes));
    }
    let mut footer_flag = b"ID3\x04\x00\x10\x00\x00\x00\x05".to_vec();
    footer_flag.extend_from_slice(&mp3);
    cases.push(("id3 with footer flag".into(), footer_flag));
    let mut xing_edge = vec![0xffu8, 0xfb, 0x90, 0x64];
    xing_edge.extend(vec![0u8; 33]);
    xing_edge.extend_from_slice(b"Xing");
    cases.push(("Xing marker at end of data".into(), xing_edge.clone()));
    let mut info_frame = vec![0u8; 417];
    info_frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
    info_frame[36..40].copy_from_slice(b"Info");
    info_frame[40..44].copy_from_slice(&0xffff_ffffu32.to_be_bytes());
    let stream = [info_frame.clone(), info_frame.clone(), info_frame].concat();
    for keep in [36usize, 40, 44, 48, 60, 100, 140, 200, 417, 500, 900] {
        cases.push((format!("Info frame with all flags set, cut at {keep}"), stream[..keep].to_vec()));
    }
    for (label, bytes) in cases {
        let path = scratch.path().join("scan.mp3");
        std::fs::write(&path, &bytes).unwrap();
        for result in [
            guarded(&label, || fourfour_analysis::mp3::scan(&path).map(|t| t.frame_samples)),
            guarded(&label, || fourfour_analysis::mp3::leading_silence_frames(&path)).map(|n| Some(n)),
            guarded(&label, || decode_mono(&path).map(|m| m.samples.len()).ok().map(|n| n as u32)),
        ] {
            if let Err(problem) = result {
                failures.push(problem);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "needs model weights (prototyping/analysis-packaging/fetch-models.sh or FOURFOUR_MODELS_DIR)"]
fn damaged_audio_never_panics_or_hangs_the_analyzer() {
    let models = locate_models(None).expect("model weights");
    let analyzer = Analyzer::load(&models, PoolOptions::default()).unwrap();
    let scratch = Scratch::new("analyze");
    let encoded = Encoded::make(scratch.path());
    let files = damaged_files(scratch.path(), &encoded);
    let mut failures = Vec::new();
    for file in &files {
        match guarded(&file.label, || analyzer.analyze(&file.path)) {
            Ok(Ok(result)) => eprintln!("ANALYZE {:<34} ok bpm={:.1} key={}", file.label, result.bpm, result.key),
            Ok(Err(e)) => eprintln!("ANALYZE {:<34} err {e}", file.label),
            Err(problem) => failures.push(problem),
        }
    }
    assert!(failures.is_empty(), "{} of {} files:\n{}", failures.len(), files.len(), failures.join("\n"));
}
