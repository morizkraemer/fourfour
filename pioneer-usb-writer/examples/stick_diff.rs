use std::path::Path;
use std::process::ExitCode;

use pioneer_usb_writer::stick::dump::{diff_sticks, DumpOptions};

const USAGE: &str = "usage: stick_diff <stick a> <stick b> [--raw] [--pages] [--no-normalise]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    if roots.len() != 2 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let options = DumpOptions { raw: flag("--raw"), pages: flag("--pages"), normalise: !flag("--no-normalise") };
    match diff_sticks(Path::new(roots[0]), Path::new(roots[1]), options) {
        Ok(lines) if lines.is_empty() => ExitCode::SUCCESS,
        Ok(lines) => {
            for line in &lines {
                println!("{line}");
            }
            eprintln!("stick_diff: {} lines differ", lines.len());
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("stick_diff: {error:#}");
            ExitCode::from(2)
        }
    }
}
