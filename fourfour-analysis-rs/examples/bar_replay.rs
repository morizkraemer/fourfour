use fourfour_analysis::service::{bar_phase, bar_phase_by, BarOneRule};
use serde_json::{json, Value};

fn main() {
    let path = std::env::args().nth(1).expect("usage: bar_replay <ffa.jsonl>");
    for line in std::fs::read_to_string(path).unwrap().lines().filter(|l| !l.trim().is_empty()) {
        let mut doc: Value = serde_json::from_str(line).unwrap();
        let beats: Vec<f64> = doc["final_beats_s"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let downbeats: Vec<f32> = doc["downbeats_s"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let delay_s = doc["mp3_delay_ms"].as_f64().unwrap() / 1000.0;
        doc["bar_phase"] = json!(bar_phase(&beats, &downbeats, delay_s));
        doc["bar_phase_model_vote"] = json!(bar_phase_by(BarOneRule::ModelVote, &beats, &downbeats, delay_s));
        println!("{doc}");
    }
}
