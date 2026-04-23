/// anlz-diff — compare two ANLZ (.DAT or .EXT) files section-by-section.
///
/// Usage:
///   cargo run --bin anlz-diff -- <ours.anlz> <theirs.anlz>
///
/// Prints each section tag, length, and a hex dump of the first 64 bytes
/// of data. Highlights sections that exist in one file but not the other
/// or have different lengths.
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: anlz-diff <ours.anlz> <theirs.anlz>");
        std::process::exit(1);
    }

    let ours = std::fs::read(&args[1]).expect("Failed to read ours");
    let theirs = std::fs::read(&args[2]).expect("Failed to read theirs");

    println!("=== {} ({} bytes) ===", args[1], ours.len());
    let ours_sections = parse_sections(&ours);
    println!();
    println!("=== {} ({} bytes) ===", args[2], theirs.len());
    let theirs_sections = parse_sections(&theirs);
    println!();

    println!("=== DIFF ===");
    let all_tags: Vec<String> = ours_sections
        .keys()
        .chain(theirs_sections.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for tag in all_tags {
        let o = ours_sections.get(&tag);
        let t = theirs_sections.get(&tag);
        match (o, t) {
            (Some(o_sec), Some(t_sec)) => {
                if o_sec.len() != t_sec.len() {
                    println!("  {}  LENGTH DIFFERENT: {} vs {}", tag, o_sec.len(), t_sec.len());
                } else if o_sec != t_sec {
                    println!("  {}  DATA DIFFERENT ({} bytes)", tag, o_sec.len());
                    hex_diff(&tag, o_sec, t_sec);
                } else {
                    println!("  {}  IDENTICAL ({} bytes)", tag, o_sec.len());
                }
            }
            (Some(o_sec), None) => {
                println!("  {}  ONLY IN OURS ({} bytes)", tag, o_sec.len());
                hex_preview(&tag, o_sec);
            }
            (None, Some(t_sec)) => {
                println!("  {}  ONLY IN THEIRS ({} bytes)", tag, t_sec.len());
                hex_preview(&tag, t_sec);
            }
            (None, None) => {}
        }
    }
}

fn parse_sections(data: &[u8]) -> std::collections::HashMap<String, &[u8]> {
    let mut map = std::collections::HashMap::new();
    if data.len() < 28 {
        return map;
    }
    let magic = std::str::from_utf8(&data[0..4]).unwrap_or("????");
    println!("Magic: {}", magic);
    let file_len = u32::from_be_bytes(data[8..12].try_into().unwrap());
    println!("File len: {}", file_len);

    let mut offset = 28usize;
    while offset + 12 <= data.len() {
        let tag = std::str::from_utf8(&data[offset..offset + 4]).unwrap_or("????").to_string();
        let header_len = u32::from_be_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let section_len = u32::from_be_bytes(data[offset + 8..offset + 12].try_into().unwrap()) as usize;
        println!("  {}  header={}  total={}", tag, header_len, section_len);
        let end = (offset + section_len).min(data.len());
        map.insert(tag, &data[offset..end]);
        offset = end;
    }
    map
}

fn hex_preview(tag: &str, data: &[u8]) {
    let show = data.len().min(128);
    for i in (0..show).step_by(16) {
        let end = (i + 16).min(show);
        let hex: Vec<String> = data[i..end].iter().map(|b| format!("{:02x}", b)).collect();
        let ascii: String = data[i..end]
            .iter()
            .map(|b| if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '.' })
            .collect();
        println!("    {:04x}  {:48}  {}", i, hex.join(" "), ascii);
    }
    if data.len() > 128 {
        println!("    ... ({} more bytes)", data.len() - 128);
    }
}

fn hex_diff(tag: &str, a: &[u8], b: &[u8]) {
    let show = a.len().min(b.len()).min(128);
    for i in (0..show).step_by(16) {
        let end = (i + 16).min(show);
        let a_hex: Vec<String> = a[i..end].iter().map(|b| format!("{:02x}", b)).collect();
        let b_hex: Vec<String> = b[i..end].iter().map(|b| format!("{:02x}", b)).collect();
        let same = a[i..end] == b[i..end];
        let marker = if same { "  " } else { "<>" };
        println!("    {} {:04x}  ours:   {:48}", marker, i, a_hex.join(" "));
        println!("    {} {:04x}  theirs: {:48}", marker, i, b_hex.join(" "));
    }
}
