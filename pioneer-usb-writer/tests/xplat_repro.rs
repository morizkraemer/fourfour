use pioneer_usb_writer::models::Track;
use pioneer_usb_writer::writer::pdb::write_pdb;
use std::path::PathBuf;

fn track(source: &str) -> Track {
    Track {
        source_path: PathBuf::from(source),
        usb_path: "/Contents/Artist/same.mp3".into(),
        title: "T".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        genre: "House".into(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: 1,
        tempo: 12000,
        key: "8A".into(),
        duration_secs: 200.0,
        sample_rate: 44100,
        bitrate: 320,
        file_size: 1000,
        id: 1,
        artwork: None,
    }
}

#[test]
#[ignore = "repro, USB writer lane: write_pdb matches the file extension case-sensitively, so Track.MP3 gets file_type 0x00 (unknown) while the scanner accepts it; lowercase the extension in pdb.rs"]
fn pdb_file_type_ignores_extension_case() {
    let dir = std::env::temp_dir().join(format!("xplat-pdb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lower = dir.join("lower.pdb");
    let upper = dir.join("upper.pdb");
    write_pdb(&lower, &[track("/music/same.mp3")], &[]).unwrap();
    write_pdb(&upper, &[track("/music/same.MP3")], &[]).unwrap();
    let equal = std::fs::read(&lower).unwrap() == std::fs::read(&upper).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(equal, "PDB differs between .mp3 and .MP3 sources");
}
