use std::path::Path;
use std::process::ExitCode;

use pioneer_usb_writer::stick::dump::{dump_stick, DumpOptions};

const USAGE: &str = "usage: stick_dump <stick root> [--raw] [--pages] [--no-normalise]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    if roots.len() != 1 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let options = DumpOptions { raw: flag("--raw"), pages: flag("--pages"), normalise: !flag("--no-normalise") };
    match dump_stick(Path::new(roots[0]), options) {
        Ok(dump) => {
            print!("{dump}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("stick_dump: {error:#}");
            ExitCode::from(2)
        }
    }
}
