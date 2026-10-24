use std::path::PathBuf;

use fourfour_analysis::service::{analyze_batch, locate_models, Analyzer, PoolOptions};

fn main() -> anyhow::Result<()> {
    let mut max_threads = None;
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--threads" => max_threads = Some(args.next().expect("--threads N").parse()?),
            "--list" => {
                let text = std::fs::read_to_string(args.next().expect("--list file"))?;
                paths.extend(text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let dir = locate_models(None).ok_or_else(|| anyhow::anyhow!("model weights not found; set FOURFOUR_MODELS_DIR"))?;
    let defaults = PoolOptions::default();
    let analyzer = Analyzer::load(&dir, PoolOptions { max_threads: max_threads.unwrap_or(defaults.max_threads), ..defaults })?;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let results = analyze_batch(&analyzer, &paths, &cancel, |e| eprintln!("[{}/{}] {}", e.done, e.total, e.path.display()));
    let out: Vec<serde_json::Value> = paths
        .iter()
        .zip(results)
        .map(|(path, result)| match result {
            Some(Ok(r)) => serde_json::json!({ "path": path, "result": r }),
            Some(Err(e)) => serde_json::json!({ "path": path, "error": e.to_string() }),
            None => serde_json::json!({ "path": path, "error": "cancelled" }),
        })
        .collect();
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
