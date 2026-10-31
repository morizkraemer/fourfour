use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ffcore::cyankey;
use ffcore::keymap::class_from_camelot;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let manifest = PathBuf::from(&args[0]);
    let audio_dir = PathBuf::from(&args[1]);
    let threads: usize = args[2].parse().unwrap();
    let out_dir = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&out_dir).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    let entries: Vec<(usize, String, usize)> = json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap().parse().unwrap(),
                e["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string(),
                class_from_camelot(e["ground_truth"]["key"].as_str().unwrap()).unwrap(),
            )
        })
        .collect();
    let next = AtomicUsize::new(0);
    let index = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= entries.len() {
                    break;
                }
                let (id, file, truth) = &entries[i];
                let audio = beat_this::load_audio(&audio_dir.join(file), 44100).expect("decode");
                let Some(b) = cyankey::semitone_bands(&audio.samples, 44100, 1) else { continue };
                let bytes: Vec<u8> = b.rows.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(out_dir.join(format!("{id}.f32")), bytes).unwrap();
                index.lock().unwrap().push(serde_json::json!({
                    "id": id, "truth": truth, "frames": b.nb_frames, "tuning": b.tuning, "fs": b.fs
                }));
            });
        }
    });
    let index = index.into_inner().unwrap();
    std::fs::write(out_dir.join("index.json"), serde_json::to_string(&index).unwrap()).unwrap();
    println!("{} tracks", index.len());
}
