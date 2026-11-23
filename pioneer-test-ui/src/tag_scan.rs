use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result};
use lofty::config::ParseOptions;
use lofty::file::FileType;
use pioneer_usb_writer::models::Track;
use pioneer_usb_writer::scanner::{check_tag_structure, track_from_reader};

const ID3_HEADER_LEN: u64 = 10;
const MAX_GAPS: usize = 64;
const MAX_RIFF_CHUNKS: usize = 256;
const READ_BUFFER_LEN: usize = 8 * 1024;

struct GapReader {
    file: File,
    gaps: Vec<Range<u64>>,
    len: u64,
    pos: u64,
    file_pos: u64,
}

impl GapReader {
    fn new(mut file: File, gaps: Vec<Range<u64>>) -> io::Result<Self> {
        file.seek(SeekFrom::Start(0))?;
        let len = file.metadata()?.len();
        let gaps = gaps.into_iter().filter(|g| g.start < g.end && g.end <= len).collect();
        Ok(Self { file, gaps, len, pos: 0, file_pos: 0 })
    }
}

impl Read for GapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if let Some(gap) = self.gaps.iter().find(|g| g.contains(&self.pos)) {
            let n = (gap.end - self.pos).min(buf.len() as u64) as usize;
            buf[..n].fill(0);
            self.pos += n as u64;
            return Ok(n);
        }
        let next_gap = self.gaps.iter().filter(|g| g.start > self.pos).map(|g| g.start).min();
        let want = next_gap.map_or(buf.len() as u64, |start| (start - self.pos).min(buf.len() as u64)) as usize;
        if self.file_pos != self.pos {
            self.file.seek(SeekFrom::Start(self.pos))?;
            self.file_pos = self.pos;
        }
        let n = self.file.read(&mut buf[..want])?;
        self.pos += n as u64;
        self.file_pos += n as u64;
        Ok(n)
    }
}

impl Seek for GapReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
        };
        self.pos = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.pos)
    }
}

fn read_at(file: &mut File, pos: u64, buf: &mut [u8]) -> io::Result<()> {
    file.seek(SeekFrom::Start(pos))?;
    file.read_exact(buf)
}

fn bad_layout() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "unexpected layout")
}

fn syncsafe(b: &[u8]) -> io::Result<u64> {
    if b.iter().any(|&x| x >= 0x80) {
        return Err(bad_layout());
    }
    Ok(b.iter().fold(0u64, |acc, &x| (acc << 7) | u64::from(x)))
}

fn id3_picture_gaps(file: &mut File, tag_start: u64, file_len: u64) -> io::Result<Vec<Range<u64>>> {
    let mut header = [0u8; ID3_HEADER_LEN as usize];
    read_at(file, tag_start, &mut header)?;
    let version = header[3];
    let flags = header[5];
    if &header[..3] != b"ID3" || !(2..=4).contains(&version) || flags & 0xC0 != 0 {
        return Err(bad_layout());
    }
    let tag_end = tag_start + ID3_HEADER_LEN + syncsafe(&header[6..10])?;
    if tag_end > file_len {
        return Err(bad_layout());
    }
    let frame_header_len: u64 = if version == 2 { 6 } else { 10 };
    let mut gaps = Vec::new();
    let mut pos = tag_start + ID3_HEADER_LEN;
    while pos + frame_header_len <= tag_end {
        let mut fh = [0u8; 10];
        read_at(file, pos, &mut fh[..frame_header_len as usize])?;
        if fh[0] == 0 {
            break;
        }
        let (id, size) = match version {
            2 => (&fh[..3], u64::from(u32::from_be_bytes([0, fh[3], fh[4], fh[5]]))),
            3 => (&fh[..4], u64::from(u32::from_be_bytes([fh[4], fh[5], fh[6], fh[7]]))),
            _ => (&fh[..4], syncsafe(&fh[4..8])?),
        };
        if !id.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
            return Err(bad_layout());
        }
        let data_start = pos + frame_header_len;
        let frame_end = data_start + size;
        if frame_end > tag_end {
            return Err(bad_layout());
        }
        if (id == b"APIC" || id == b"PIC") && size > 0 {
            if gaps.len() == MAX_GAPS {
                return Err(bad_layout());
            }
            gaps.push(data_start..frame_end);
        }
        pos = frame_end;
    }
    Ok(gaps)
}

fn riff_id3_picture_gaps(file: &mut File, file_len: u64, big_endian: bool) -> io::Result<Vec<Range<u64>>> {
    let mut pos = 12;
    let mut seen = 0;
    while pos + 8 <= file_len {
        seen += 1;
        if seen > MAX_RIFF_CHUNKS {
            return Err(bad_layout());
        }
        let mut ch = [0u8; 8];
        read_at(file, pos, &mut ch)?;
        let raw = [ch[4], ch[5], ch[6], ch[7]];
        let size = u64::from(if big_endian { u32::from_be_bytes(raw) } else { u32::from_le_bytes(raw) });
        if ch[..4].eq_ignore_ascii_case(b"id3 ") {
            return id3_picture_gaps(file, pos + 8, file_len);
        }
        pos += 8 + size + (size & 1);
    }
    Ok(Vec::new())
}

struct Atom {
    kind: [u8; 4],
    header_len: u64,
    start: u64,
    end: u64,
}

fn atoms(file: &mut File, from: u64, to: u64) -> io::Result<Vec<Atom>> {
    let mut out = Vec::new();
    let mut pos = from;
    while pos + 8 <= to {
        let mut h = [0u8; 16];
        read_at(file, pos, &mut h[..8])?;
        let mut size = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let mut header_len = 8;
        if size == 1 {
            read_at(file, pos + 8, &mut h[8..16])?;
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
            header_len = 16;
        } else if size == 0 {
            size = to - pos;
        }
        if size < header_len || pos + size > to || out.len() > 4096 {
            return Err(bad_layout());
        }
        out.push(Atom { kind: [h[4], h[5], h[6], h[7]], header_len, start: pos, end: pos + size });
        pos += size;
    }
    Ok(out)
}

fn child(file: &mut File, parent_body: Range<u64>, kind: &[u8; 4]) -> io::Result<Atom> {
    atoms(file, parent_body.start, parent_body.end)?.into_iter().find(|a| &a.kind == kind).ok_or_else(bad_layout)
}

fn mp4_picture_gaps(file: &mut File, file_len: u64) -> io::Result<Vec<Range<u64>>> {
    let moov = child(file, 0..file_len, b"moov")?;
    let udta = child(file, moov.start + moov.header_len..moov.end, b"udta")?;
    let meta = child(file, udta.start + udta.header_len..udta.end, b"meta")?;
    let ilst = child(file, meta.start + meta.header_len + 4..meta.end, b"ilst")?;
    let gaps = atoms(file, ilst.start + ilst.header_len, ilst.end)?
        .into_iter()
        .filter(|a| &a.kind == b"covr")
        .map(|a| a.start + a.header_len..a.end)
        .collect();
    Ok(gaps)
}

fn picture_gaps(file: &mut File, file_type: FileType) -> Vec<Range<u64>> {
    let Ok(len) = file.metadata().map(|m| m.len()) else { return Vec::new() };
    let gaps = match file_type {
        FileType::Mpeg => id3_picture_gaps(file, 0, len),
        FileType::Aiff => riff_id3_picture_gaps(file, len, true),
        FileType::Wav => riff_id3_picture_gaps(file, len, false),
        FileType::Mp4 => mp4_picture_gaps(file, len),
        _ => Ok(Vec::new()),
    };
    gaps.unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn advise_random_access(file: &File) {
    use std::os::fd::AsRawFd;
    unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_RANDOM) };
}

#[cfg(not(target_os = "linux"))]
fn advise_random_access(_file: &File) {}

pub fn read_track_tags(path: &Path, id: u32) -> Result<Track> {
    let is_regular = std::fs::metadata(path).map(|m| m.is_file()).with_context(|| format!("Failed to open {}", path.display()))?;
    anyhow::ensure!(is_regular, "{} is not a regular file", path.display());
    let mut file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    advise_random_access(&file);
    check_tag_structure(&mut file).with_context(|| format!("Refusing to read tags from {}", path.display()))?;
    let file_type = FileType::from_path(path);
    let gaps = file_type.map_or_else(Vec::new, |ft| picture_gaps(&mut file, ft));
    let has_gap_support = matches!(file_type, Some(FileType::Mpeg | FileType::Wav | FileType::Aiff | FileType::Mp4));
    let track = if !gaps.is_empty() {
        let reader = GapReader::new(file, gaps).with_context(|| format!("Failed to open {}", path.display()))?;
        track_from_reader(&mut BufReader::with_capacity(READ_BUFFER_LEN, reader), ParseOptions::new().read_cover_art(false), path, id)?
    } else {
        file.seek(SeekFrom::Start(0))?;
        let options = if has_gap_support { ParseOptions::new() } else { ParseOptions::new().read_cover_art(false) };
        track_from_reader(&mut BufReader::new(file), options, path, id)?
    };
    Ok(Track { artwork: None, ..track })
}

pub fn read_track_with_cover(path: &Path, id: u32) -> Result<Track> {
    let is_regular = std::fs::metadata(path).map(|m| m.is_file()).with_context(|| format!("Failed to open {}", path.display()))?;
    anyhow::ensure!(is_regular, "{} is not a regular file", path.display());
    let mut file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    advise_random_access(&file);
    check_tag_structure(&mut file).with_context(|| format!("Refusing to read tags from {}", path.display()))?;
    track_from_reader(&mut BufReader::new(file), ParseOptions::new(), path, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::scanner::read_track_metadata;
    use std::io::Write;

    fn id3_frame(id: &[u8], body: &[u8], version: u8) -> Vec<u8> {
        let mut f = id.to_vec();
        if version == 4 {
            f.extend((0..4).rev().map(|i| ((body.len() >> (7 * i)) & 0x7f) as u8));
        } else {
            f.extend_from_slice(&(body.len() as u32).to_be_bytes());
        }
        f.extend_from_slice(&[0, 0]);
        f.extend_from_slice(body);
        f
    }

    fn text_frame(id: &[u8], text: &str, version: u8) -> Vec<u8> {
        let mut body = vec![3u8];
        body.extend_from_slice(text.as_bytes());
        id3_frame(id, &body, version)
    }

    fn apic_frame(cover: &[u8], version: u8) -> Vec<u8> {
        let mut body = vec![0u8];
        body.extend_from_slice(b"image/jpeg\0");
        body.push(3);
        body.push(0);
        body.extend_from_slice(cover);
        id3_frame(b"APIC", &body, version)
    }

    fn mp3_with_tag(version: u8, cover_len: usize, padding: usize) -> Vec<u8> {
        let mut frames = text_frame(b"TIT2", "A Title", version);
        frames.extend(apic_frame(&vec![0xAB; cover_len], version));
        frames.extend(text_frame(b"TPE1", "An Artist", version));
        frames.extend(text_frame(b"TALB", "An Album", version));
        frames.extend(vec![0u8; padding]);
        let mut out = b"ID3".to_vec();
        out.extend_from_slice(&[version, 0, 0]);
        out.extend((0..4).rev().map(|i| ((frames.len() >> (7 * i)) & 0x7f) as u8));
        out.extend(frames);
        for _ in 0..40 {
            out.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            out.extend(vec![0u8; 413]);
        }
        out
    }

    fn same_as_full_read(name: &str, bytes: &[u8]) {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let full = read_track_metadata(&path, 7).map(|t| Track { artwork: None, ..t });
        let quick = read_track_tags(&path, 7);
        std::fs::remove_dir_all(&dir).unwrap();
        match (full, quick) {
            (Ok(full), Ok(quick)) => assert_eq!(format!("{full:?}"), format!("{quick:?}"), "{name}"),
            (Err(_), Err(_)) => {}
            (full, quick) => panic!("{name}: full {:?}, quick {:?}", full.is_ok(), quick.is_ok()),
        }
    }

    #[test]
    fn id3_tags_read_without_the_cover_match_the_full_read() {
        for version in [3u8, 4] {
            for (cover_len, padding) in [(0usize, 0usize), (1, 0), (5000, 0), (300_000, 100), (70_000, 20)] {
                same_as_full_read(&format!("v{version}-{cover_len}-{padding}.mp3"), &mp3_with_tag(version, cover_len, padding));
            }
        }
    }

    #[test]
    fn id3_gaps_cover_exactly_the_picture_bodies() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-gaps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("g.mp3");
        let bytes = mp3_with_tag(4, 1000, 0);
        std::fs::write(&path, &bytes).unwrap();
        let mut file = File::open(&path).unwrap();
        let gaps = id3_picture_gaps(&mut file, 0, bytes.len() as u64).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].end - gaps[0].start, 1000 + 1 + 11 + 2);
        assert!(bytes[gaps[0].start as usize..gaps[0].end as usize].starts_with(&[0, b'i']));
    }

    #[test]
    fn unsynchronised_or_broken_tags_fall_back_to_the_full_read() {
        let mut unsync = mp3_with_tag(3, 2000, 0);
        unsync[5] = 0x80;
        same_as_full_read("unsync.mp3", &unsync);
        let mut truncated = mp3_with_tag(4, 4000, 0);
        truncated.truncate(2500);
        same_as_full_read("truncated.mp3", &truncated);
        let mut garbage = mp3_with_tag(4, 4000, 0);
        garbage[10] = b'#';
        same_as_full_read("garbage.mp3", &garbage);
    }

    #[test]
    fn mp4_cover_atoms_are_zeroed_and_tags_stay() {
        let atom = |kind: &[u8; 4], body: &[u8]| {
            let mut a = ((body.len() + 8) as u32).to_be_bytes().to_vec();
            a.extend_from_slice(kind);
            a.extend_from_slice(body);
            a
        };
        let data = |flags: u8, payload: &[u8]| {
            let mut b = vec![0, 0, 0, flags, 0, 0, 0, 0];
            b.extend_from_slice(payload);
            atom(b"data", &b)
        };
        let mut ilst = atom(&[0xA9, b'n', b'a', b'm'], &data(1, b"M4A Title"));
        ilst.extend(atom(&[0xA9, b'A', b'R', b'T'], &data(1, b"M4A Artist")));
        ilst.extend(atom(b"covr", &data(13, &vec![0xCD; 50_000])));
        let mut meta_body = vec![0, 0, 0, 0];
        meta_body.extend(atom(b"ilst", &ilst));
        let udta = atom(b"udta", &atom(b"meta", &meta_body));
        let mut file = atom(b"ftyp", b"M4A \0\0\0\0M4A mp42isom");
        file.extend(atom(b"mdat", &vec![0u8; 3000]));
        file.extend(atom(b"moov", &udta));
        same_as_full_read("a.m4a", &file);

        let dir = std::env::temp_dir().join(format!("ff-tagscan-mp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.m4a");
        std::fs::write(&path, &file).unwrap();
        let mut f = File::open(&path).unwrap();
        let gaps = mp4_picture_gaps(&mut f, file.len() as u64).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].end - gaps[0].start, 50_000 + 16);
    }

    #[test]
    fn gap_reader_serves_zeros_for_gaps_and_the_file_elsewhere() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-reader-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.bin");
        let bytes: Vec<u8> = (0..100u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        let mut r = GapReader::new(File::open(&path).unwrap(), vec![10..20, 50..60]).unwrap();
        let mut all = Vec::new();
        r.read_to_end(&mut all).unwrap();
        let mut expected = bytes.clone();
        expected[10..20].fill(0);
        expected[50..60].fill(0);
        assert_eq!(all, expected);
        r.seek(SeekFrom::Start(15)).unwrap();
        let mut buf = [9u8; 10];
        r.read_exact(&mut buf).unwrap();
        assert_eq!(&buf[..5], &[0; 5]);
        assert_eq!(&buf[5..], &bytes[20..25]);
        assert_eq!(r.seek(SeekFrom::End(-1)).unwrap(), 99);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sparse_zero_files_read_as_fast_as_the_plain_reader() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-sparse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["mp3", "wav", "aiff", "m4a", "flac"] {
            let path = dir.join(format!("zeros.{ext}"));
            std::fs::File::create(&path).unwrap().set_len(64 << 20).unwrap();
            let started = std::time::Instant::now();
            let quick = read_track_tags(&path, 1);
            let quick_secs = started.elapsed().as_secs_f64();
            let started = std::time::Instant::now();
            let full = read_track_metadata(&path, 1);
            let full_secs = started.elapsed().as_secs_f64();
            assert_eq!(quick.is_ok(), full.is_ok(), "{ext}");
            assert!(quick_secs < full_secs * 4.0 + 1.0, "{ext}: {quick_secs:.2}s against {full_secs:.2}s");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mp3_with_a_real_head_and_a_sparse_body_reads_as_fast_as_the_plain_reader() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-head-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("head.mp3");
        let mut file = std::fs::File::create(&path).unwrap();
        let frames = text_frame(b"TIT2", "A Title", 4);
        let mut head = b"ID3".to_vec();
        head.extend_from_slice(&[4, 0, 0]);
        head.extend((0..4).rev().map(|i| ((frames.len() >> (7 * i)) & 0x7f) as u8));
        head.extend(frames);
        for _ in 0..4 {
            head.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            head.extend(vec![0u8; 413]);
        }
        file.write_all(&head).unwrap();
        file.set_len(64 << 20).unwrap();
        let started = std::time::Instant::now();
        let quick = read_track_tags(&path, 1);
        let quick_secs = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        let full = read_track_metadata(&path, 1);
        let full_secs = started.elapsed().as_secs_f64();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(quick.is_ok(), full.is_ok());
        assert!(quick_secs < full_secs * 4.0 + 1.0, "{quick_secs:.2}s against {full_secs:.2}s");
    }

    #[test]
    fn special_files_are_refused_without_opening_them() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-special-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fifo = dir.join("pipe.mp3");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status().map(|s| s.success()).unwrap_or(false);
        if made {
            assert!(read_track_tags(&fifo, 1).is_err());
        }
        assert!(read_track_tags(Path::new("/dev/zero"), 1).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn big_cover() -> Vec<u8> {
        (0..400_000u32).map(|i| (i.wrapping_mul(31) >> 3) as u8).collect()
    }

    fn id3_tag_with_cover(cover: &[u8]) -> Vec<u8> {
        let mut frames = text_frame(b"TIT2", "A Title", 4);
        frames.extend(apic_frame(cover, 4));
        frames.extend(text_frame(b"TPE1", "An Artist", 4));
        frames.extend(text_frame(b"TALB", "An Album", 4));
        let mut out = b"ID3".to_vec();
        out.extend_from_slice(&[4, 0, 0]);
        out.extend((0..4).rev().map(|i| ((frames.len() >> (7 * i)) & 0x7f) as u8));
        out.extend(frames);
        out
    }

    fn flac_with_cover(cover: &[u8]) -> Vec<u8> {
        let block = |kind: u8, last: bool, body: &[u8]| {
            let mut b = vec![kind | if last { 0x80 } else { 0 }];
            b.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
            b.extend_from_slice(body);
            b
        };
        let mut info = Vec::new();
        info.extend_from_slice(&4096u16.to_be_bytes());
        info.extend_from_slice(&4096u16.to_be_bytes());
        info.extend_from_slice(&[0; 6]);
        info.extend_from_slice(&((44100u32 << 12) | (1 << 9) | (15 << 4)).to_be_bytes());
        info.extend_from_slice(&441_000u32.to_be_bytes());
        info.extend_from_slice(&[0; 16]);
        let mut vorbis = Vec::new();
        vorbis.extend_from_slice(&4u32.to_le_bytes());
        vorbis.extend_from_slice(b"test");
        let comments = ["TITLE=A Title", "ARTIST=An Artist", "ALBUM=An Album"];
        vorbis.extend_from_slice(&(comments.len() as u32).to_le_bytes());
        for c in comments {
            vorbis.extend_from_slice(&(c.len() as u32).to_le_bytes());
            vorbis.extend_from_slice(c.as_bytes());
        }
        let mut picture = Vec::new();
        picture.extend_from_slice(&3u32.to_be_bytes());
        picture.extend_from_slice(&10u32.to_be_bytes());
        picture.extend_from_slice(b"image/jpeg");
        picture.extend_from_slice(&0u32.to_be_bytes());
        picture.extend_from_slice(&[0; 16]);
        picture.extend_from_slice(&(cover.len() as u32).to_be_bytes());
        picture.extend_from_slice(cover);
        let mut out = b"fLaC".to_vec();
        out.extend(block(0, false, &info));
        out.extend(block(4, false, &vorbis));
        out.extend(block(6, true, &picture));
        out.extend(vec![0u8; 2000]);
        out
    }

    fn aiff_with_cover(cover: &[u8]) -> Vec<u8> {
        let chunk = |id: &[u8; 4], body: &[u8]| {
            let mut c = id.to_vec();
            c.extend_from_slice(&(body.len() as u32).to_be_bytes());
            c.extend_from_slice(body);
            if body.len() % 2 == 1 {
                c.push(0);
            }
            c
        };
        let mut comm = Vec::new();
        comm.extend_from_slice(&2u16.to_be_bytes());
        comm.extend_from_slice(&100u32.to_be_bytes());
        comm.extend_from_slice(&16u16.to_be_bytes());
        comm.extend_from_slice(&[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
        let mut ssnd = vec![0u8; 8];
        ssnd.extend(vec![0u8; 400]);
        let mut body = b"AIFF".to_vec();
        body.extend(chunk(b"COMM", &comm));
        body.extend(chunk(b"SSND", &ssnd));
        body.extend(chunk(b"ID3 ", &id3_tag_with_cover(cover)));
        let mut out = b"FORM".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    fn wav_with_cover(cover: &[u8]) -> Vec<u8> {
        let chunk = |id: &[u8; 4], body: &[u8]| {
            let mut c = id.to_vec();
            c.extend_from_slice(&(body.len() as u32).to_le_bytes());
            c.extend_from_slice(body);
            if body.len() % 2 == 1 {
                c.push(0);
            }
            c
        };
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&44100u32.to_le_bytes());
        fmt.extend_from_slice(&176_400u32.to_le_bytes());
        fmt.extend_from_slice(&4u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        let mut body = b"WAVE".to_vec();
        body.extend(chunk(b"fmt ", &fmt));
        body.extend(chunk(b"data", &vec![0u8; 400]));
        body.extend(chunk(b"id3 ", &id3_tag_with_cover(cover)));
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    fn assert_tags_equal_and_cover_kept(name: &str, bytes: &[u8], cover: &[u8]) {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-cover-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let full = read_track_metadata(&path, 7).unwrap();
        let quick = read_track_tags(&path, 7).unwrap();
        let with_cover = read_track_with_cover(&path, 7).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(format!("{full:?}"), format!("{with_cover:?}"), "{name}: the cover read matches the full read");
        assert_eq!(full.title, "A Title", "{name}");
        assert_eq!(full.artwork.as_deref(), Some(cover), "{name}: the full read keeps the cover bytes");
        assert_eq!(quick.artwork, None, "{name}");
        assert_eq!(format!("{:?}", Track { artwork: None, ..full }), format!("{quick:?}"), "{name}");
    }

    #[test]
    fn large_covers_in_every_format_leave_the_tags_and_the_stored_cover_unchanged() {
        let cover = big_cover();
        assert_tags_equal_and_cover_kept("c.mp3", &[id3_tag_with_cover(&cover), (0..40).flat_map(|_| [vec![0xFF, 0xFB, 0x90, 0x00], vec![0u8; 413]].concat()).collect()].concat(), &cover);
        assert_tags_equal_and_cover_kept("c.flac", &flac_with_cover(&cover), &cover);
        assert_tags_equal_and_cover_kept("c.aiff", &aiff_with_cover(&cover), &cover);
        assert_tags_equal_and_cover_kept("c.wav", &wav_with_cover(&cover), &cover);
    }

    #[test]
    fn advising_random_access_is_harmless_on_any_file() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-advise-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let mut file = File::open(&path).unwrap();
        advise_random_access(&file);
        advise_random_access(&file);
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(text, "abc");
    }

    #[test]
    fn tag_floods_are_refused_by_the_import_reader_too() {
        let dir = std::env::temp_dir().join(format!("ff-tagscan-flood-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("flood.flac");
        let mut bytes = b"fLaC".to_vec();
        for _ in 0..5000 {
            bytes.extend_from_slice(&[0x02, 0, 0, 4, b'f', b'u', b'z', b'z']);
        }
        std::fs::write(&path, &bytes).unwrap();
        let quick = read_track_tags(&path, 1);
        let cover = read_track_with_cover(&path, 1);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(quick.is_err());
        assert!(cover.is_err());
    }
}
