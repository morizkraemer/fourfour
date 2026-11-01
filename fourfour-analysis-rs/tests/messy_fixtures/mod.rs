use std::path::{Path, PathBuf};
use std::process::Command;

pub const RATE: u32 = 44_100;
const SECONDS: f64 = 6.0;
const BPM: f64 = 120.0;

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("ff-messy-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = make_tree_writable(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_tree_writable(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755))?;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            let _ = make_tree_writable(&entry.path());
        } else if !kind.is_symlink() {
            let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o644));
        }
    }
    Ok(())
}

pub fn kick_pcm() -> Vec<i16> {
    let beat = RATE as f64 * 60.0 / BPM;
    (0..(RATE as f64 * SECONDS) as usize)
        .map(|i| {
            let since = (i as f64 % beat) / RATE as f64;
            let kick = (-since * 25.0).exp() * (2.0 * std::f64::consts::PI * (50.0 + 80.0 * (-since * 40.0).exp()) * since).sin();
            (kick * 0.8 * i16::MAX as f64) as i16
        })
        .collect()
}

fn riff_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

pub fn wav_bytes(pcm: &[i16], extra_chunks: &[u8]) -> Vec<u8> {
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&RATE.to_le_bytes());
    fmt.extend_from_slice(&(RATE * 2).to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let data: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut body = b"WAVE".to_vec();
    body.extend(riff_chunk(b"fmt ", &fmt));
    body.extend_from_slice(extra_chunks);
    body.extend(riff_chunk(b"data", &data));
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

pub fn list_info(entries: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let mut body = b"INFO".to_vec();
    for (id, value) in entries {
        let mut text = value.to_vec();
        text.push(0);
        body.extend(riff_chunk(id, &text));
    }
    riff_chunk(b"LIST", &body)
}

pub fn aiff_bytes(pcm: &[i16]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    }
    let mut comm = Vec::new();
    comm.extend_from_slice(&1u16.to_be_bytes());
    comm.extend_from_slice(&(pcm.len() as u32).to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
    let mut ssnd = vec![0u8; 8];
    ssnd.extend(pcm.iter().flat_map(|s| s.to_be_bytes()));
    let mut body = b"AIFF".to_vec();
    body.extend(chunk(b"COMM", &comm));
    body.extend(chunk(b"SSND", &ssnd));
    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

fn ffmpeg_encode(wav: &Path, out: &Path, extra: &[&str]) -> bool {
    Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(wav)
        .args(extra)
        .arg(out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn id3v2_size(head: &[u8]) -> usize {
    if head.len() >= 10 && &head[..3] == b"ID3" {
        10 + (((head[6] & 0x7f) as usize) << 21 | ((head[7] & 0x7f) as usize) << 14 | ((head[8] & 0x7f) as usize) << 7 | (head[9] & 0x7f) as usize)
    } else {
        0
    }
}

pub struct Encoded {
    pub wav: Vec<u8>,
    pub aiff: Vec<u8>,
    pub mp3: Option<Vec<u8>>,
    pub flac: Option<Vec<u8>>,
    pub m4a: Option<Vec<u8>>,
}

impl Encoded {
    pub fn make(work: &Path) -> Encoded {
        let pcm = kick_pcm();
        let wav = wav_bytes(&pcm, &[]);
        let source = work.join("source.wav");
        std::fs::write(&source, &wav).unwrap();
        let encode = |name: &str, extra: &[&str]| {
            let out = work.join(name);
            ffmpeg_encode(&source, &out, extra).then(|| std::fs::read(&out).unwrap())
        };
        let mp3 = encode("source.mp3", &["-write_id3v2", "0", "-write_xing", "0"]).map(|bytes| bytes[id3v2_size(&bytes)..].to_vec());
        Encoded { aiff: aiff_bytes(&pcm), mp3, flac: encode("source.flac", &[]), m4a: encode("source.m4a", &["-c:a", "aac"]), wav }
    }

    pub fn by_ext(&self, ext: &str) -> Option<&[u8]> {
        match ext {
            "wav" => Some(&self.wav),
            "aiff" | "aif" => Some(&self.aiff),
            "mp3" => self.mp3.as_deref(),
            "flac" => self.flac.as_deref(),
            "m4a" => self.m4a.as_deref(),
            _ => None,
        }
    }
}

pub const FORMATS: [&str; 5] = ["wav", "aiff", "mp3", "flac", "m4a"];

fn syncsafe(n: usize) -> [u8; 4] {
    [((n >> 21) & 0x7f) as u8, ((n >> 14) & 0x7f) as u8, ((n >> 7) & 0x7f) as u8, (n & 0x7f) as u8]
}

pub fn id3v24_frame(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    id3v24_frame_claiming(id, payload, payload.len())
}

pub fn id3v24_frame_claiming(id: &[u8; 4], payload: &[u8], claimed: usize) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&syncsafe(claimed));
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(payload);
    out
}

pub fn text_frame(id: &[u8; 4], encoding: u8, text: &[u8]) -> Vec<u8> {
    let mut payload = vec![encoding];
    payload.extend_from_slice(text);
    id3v24_frame(id, &payload)
}

pub fn with_id3v24(frames: &[u8], audio: &[u8]) -> Vec<u8> {
    with_id3v24_claiming(frames, frames.len(), audio)
}

pub fn with_id3v24_claiming(frames: &[u8], claimed: usize, audio: &[u8]) -> Vec<u8> {
    let mut out = b"ID3".to_vec();
    out.extend_from_slice(&[4, 0, 0]);
    out.extend_from_slice(&syncsafe(claimed));
    out.extend_from_slice(frames);
    out.extend_from_slice(audio);
    out
}

pub fn utf16le(text: &[u16]) -> Vec<u8> {
    let mut out = vec![0xff, 0xfe];
    out.extend(text.iter().flat_map(|u| u.to_le_bytes()));
    out
}

pub const EMOJI_RTL_TITLE: &str = "\u{1F3A7}\u{1F525} \u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{5E2}\u{5D5}\u{5DC}\u{5DD} \u{645}\u{631}\u{62D}\u{628}\u{627} e\u{301}\u{200B}\u{202E}x\u{FEFF}";

pub struct TagCase {
    pub name: &'static str,
    pub bytes: Vec<u8>,
    pub ext: &'static str,
}

pub fn tag_cases(encoded: &Encoded) -> Vec<TagCase> {
    let mp3 = encoded.mp3.clone().unwrap_or_else(synthetic_mp3);
    let wav_with = |chunks: Vec<u8>| wav_bytes(&kick_pcm(), &chunks);
    let long = vec![b'A'; 3 * 1024 * 1024];
    let mut cases = vec![
        TagCase { name: "mp3-3mb-title", bytes: with_id3v24(&text_frame(b"TIT2", 3, &long), &mp3), ext: "mp3" },
        TagCase { name: "mp3-bad-utf8-title", bytes: with_id3v24(&[text_frame(b"TIT2", 3, &[0xff, 0xfe, 0x80, b'a', 0xc3, 0x28]), text_frame(b"TPE1", 3, &[0xc0, 0xaf, 0xed, 0xa0, 0x80])].concat(), &mp3), ext: "mp3" },
        TagCase { name: "mp3-emoji-rtl-nul", bytes: with_id3v24(&[text_frame(b"TIT2", 3, EMOJI_RTL_TITLE.as_bytes()), text_frame(b"TPE1", 3, "a\0b\u{0}c".as_bytes()), text_frame(b"TALB", 3, "\u{202E}evil".as_bytes())].concat(), &mp3), ext: "mp3" },
        TagCase { name: "mp3-lone-surrogate-utf16", bytes: with_id3v24(&text_frame(b"TIT2", 1, &utf16le(&[0xd800, 0x41, 0xdc00, 0xdc00])), &mp3), ext: "mp3" },
        TagCase { name: "mp3-frame-size-lies", bytes: with_id3v24(&id3v24_frame_claiming(b"TIT2", &[3, b'h', b'i'], 0x0fff_ffff), &mp3), ext: "mp3" },
        TagCase { name: "mp3-tag-size-lies", bytes: with_id3v24_claiming(&text_frame(b"TIT2", 3, b"short"), 0x0fff_ffff, &mp3), ext: "mp3" },
        TagCase { name: "mp3-garbage-cover", bytes: with_id3v24(&id3v24_frame(b"APIC", &[&[0u8][..], b"image/png\0", &[3], b"front\0", &[0x89, b'P', b'N', b'G', 0xff, 0xff, 0xff, 0xff, 1, 2, 3]].concat()), &mp3), ext: "mp3" },
        TagCase { name: "mp3-cover-claims-huge", bytes: with_id3v24(&id3v24_frame_claiming(b"APIC", &[0, b'i', 0, 3, 0, 1, 2, 3], 0x0fff_ff00), &mp3), ext: "mp3" },
        TagCase { name: "mp3-id3-only-no-audio", bytes: with_id3v24(&text_frame(b"TIT2", 3, b"no audio"), &[]), ext: "mp3" },
        TagCase { name: "wav-bad-utf8-info", bytes: wav_with(list_info(&[(b"INAM", &[0xff, 0xfe, 0x80, 0xc3, 0x28]), (b"IART", &[0xed, 0xa0, 0x80])])), ext: "wav" },
        TagCase { name: "wav-emoji-rtl-info", bytes: wav_with(list_info(&[(b"INAM", EMOJI_RTL_TITLE.as_bytes()), (b"IART", "\u{202E}\u{1F3B6}".as_bytes())])), ext: "wav" },
        TagCase { name: "wav-3mb-info", bytes: wav_with(list_info(&[(b"INAM", &long)])), ext: "wav" },
    ];
    let mut lying_list = b"LIST".to_vec();
    lying_list.extend_from_slice(&0x7fff_fff0u32.to_le_bytes());
    lying_list.extend_from_slice(b"INFOINAM");
    cases.push(TagCase { name: "wav-list-size-lies", bytes: wav_with(lying_list), ext: "wav" });
    for (ext, bytes) in [("flac", &encoded.flac), ("m4a", &encoded.m4a)] {
        if bytes.is_some() {
            cases.push(TagCase { name: if ext == "flac" { "flac-plain" } else { "m4a-plain" }, bytes: bytes.clone().unwrap(), ext });
        }
    }
    cases
}

pub fn synthetic_mp3() -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..200 {
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        out.extend(frame);
    }
    out
}

pub fn ffmpeg_tagged(work: &Path, ext: &str, title: &str, artist: &str) -> Option<Vec<u8>> {
    let source = work.join("source.wav");
    let out = work.join(format!("tagged.{ext}"));
    let title_arg = format!("title={title}");
    let artist_arg = format!("artist={artist}");
    let mut extra = vec!["-metadata", title_arg.as_str(), "-metadata", artist_arg.as_str()];
    if ext == "m4a" {
        extra.extend(["-c:a", "aac"]);
    }
    ffmpeg_encode(&source, &out, &extra).then(|| std::fs::read(&out).unwrap())
}

pub fn cut(bytes: &[u8], fraction: f64) -> Vec<u8> {
    bytes[..(bytes.len() as f64 * fraction) as usize].to_vec()
}

pub fn flip_bytes(bytes: &[u8], from_fraction: f64, to_fraction: f64) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let (from, to) = ((out.len() as f64 * from_fraction) as usize, (out.len() as f64 * to_fraction) as usize);
    let mut state = 0x2545_f491u32;
    for byte in &mut out[from..to] {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state as u8;
    }
    out
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9u32;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

pub struct Damaged {
    pub label: String,
    pub path: PathBuf,
}

pub fn damaged_files(dir: &Path, encoded: &Encoded) -> Vec<Damaged> {
    let mut out = Vec::new();
    let mut add = |label: String, file: String, bytes: Vec<u8>| {
        let path = dir.join(&file);
        std::fs::write(&path, bytes).unwrap();
        out.push(Damaged { label, path });
    };
    for ext in FORMATS {
        let Some(good) = encoded.by_ext(ext) else { continue };
        add(format!("zero-byte {ext}"), format!("zero.{ext}"), Vec::new());
        add(format!("1-byte {ext}"), format!("one.{ext}"), vec![0x52]);
        add(format!("header-only {ext}"), format!("header.{ext}"), good[..good.len().min(30)].to_vec());
        for fraction in [0.2, 0.5, 0.9] {
            add(format!("truncated {ext} at {fraction}"), format!("cut{}.{ext}", (fraction * 100.0) as u32), cut(good, fraction));
        }
        add(format!("middle corrupted {ext}"), format!("middle.{ext}"), flip_bytes(good, 0.3, 0.6));
        add(format!("start corrupted {ext}"), format!("start.{ext}"), flip_bytes(good, 0.0, 0.2));
        add(format!("random bytes {ext}"), format!("random.{ext}"), random_bytes(200_000));
        add(format!("text as {ext}"), format!("text.{ext}"), b"this is not audio, just words\n".repeat(500));
    }
    let wav = &encoded.wav;
    add("wav named .mp3".into(), "wav-as.mp3".into(), wav.clone());
    add("wav named .flac".into(), "wav-as.flac".into(), wav.clone());
    add("wav named .m4a".into(), "wav-as.m4a".into(), wav.clone());
    if let Some(mp3) = &encoded.mp3 {
        add("mp3 named .wav".into(), "mp3-as.wav".into(), mp3.clone());
        add("mp3 named .flac".into(), "mp3-as.flac".into(), mp3.clone());
        add("mp3 named .aiff".into(), "mp3-as.aiff".into(), mp3.clone());
    }
    add("aiff named .wav".into(), "aiff-as.wav".into(), encoded.aiff.clone());
    add("ogg-looking .ogg".into(), "fake.ogg".into(), [b"OggS".as_slice(), &random_bytes(5000)].concat());
    add("wav claiming huge data".into(), "huge-data.wav".into(), {
        let mut bytes = wav.clone();
        let data_at = bytes.windows(4).position(|w| w == b"data").unwrap();
        bytes[data_at + 4..data_at + 8].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
        bytes
    });
    add("wav with zero sample rate".into(), "zero-rate.wav".into(), {
        let mut bytes = wav.clone();
        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
        bytes
    });
    add("wav with zero channels".into(), "zero-channels.wav".into(), {
        let mut bytes = wav.clone();
        bytes[22..24].copy_from_slice(&0u16.to_le_bytes());
        bytes
    });
    add("wav with 0 bits".into(), "zero-bits.wav".into(), {
        let mut bytes = wav.clone();
        bytes[34..36].copy_from_slice(&0u16.to_le_bytes());
        bytes
    });
    add("wav with block align 0".into(), "zero-align.wav".into(), {
        let mut bytes = wav.clone();
        bytes[32..34].copy_from_slice(&0u16.to_le_bytes());
        bytes
    });
    add("wav 32 kHz mono 3 samples".into(), "tiny.wav".into(), wav_bytes(&[1, 2, 3], &[]));
    add("wav silence 2s".into(), "silence.wav".into(), wav_bytes(&vec![0i16; RATE as usize * 2], &[]));
    out
}

