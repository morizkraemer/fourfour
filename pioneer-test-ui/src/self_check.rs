use std::path::PathBuf;

use fourfour_analysis::service::{models_present, Analyzer, PoolOptions};

pub const FLAG: &str = "--self-check";

pub fn requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == FLAG)
}

fn bundled_models_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current exe: {e}"))?;
    let exe_dir = exe.parent().ok_or("executable has no parent directory")?;
    if cfg!(target_os = "macos") {
        let contents = exe_dir.parent().ok_or("executable is not inside a bundle")?;
        Ok(contents.join("Resources").join("models"))
    } else if cfg!(windows) {
        Ok(exe_dir.join("models"))
    } else {
        let prefix = exe_dir.parent().ok_or("executable is not inside an install prefix")?;
        Ok(prefix.join("lib").join("fourfour").join("models"))
    }
}

pub fn run() -> Result<String, String> {
    let dir = bundled_models_dir()?;
    if !models_present(&dir) {
        return Err(format!("bundled model weights missing in {}", dir.display()));
    }
    Analyzer::load(&dir, PoolOptions::default()).map_err(|e| format!("loading model weights from {}: {e}", dir.display()))?;
    Ok(format!("self-check ok: fourfour {} weights loaded from {}", env!("CARGO_PKG_VERSION"), dir.display()))
}

pub fn run_and_exit() -> ! {
    match run() {
        Ok(line) => {
            println!("{line}");
            std::process::exit(0);
        }
        Err(message) => {
            eprintln!("self-check failed: {message}");
            std::process::exit(1);
        }
    }
}
