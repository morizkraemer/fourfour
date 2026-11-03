use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result};
use lofty::config::ParseOptions;
use lofty::file::FileType;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use pioneer_usb_writer::models::Track;

const ID3_HEADER_LEN: u64 = 10;
const MAX_GAPS: usize = 64;

struct GapReader {
    file: BufReader<File>,
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
        Ok(Self { file: BufReader::new(file), gaps, len, pos: 0, file_pos: 0 })
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
    while pos + 8 <= file_len {
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

fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

pub fn read_track_tags(path: &Path, id: u32) -> Result<Track> {
    let mut file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let file_type = FileType::from_path(path);
    let gaps = file_type.map_or_else(Vec::new, |ft| picture_gaps(&mut file, ft));
    let reader = GapReader::new(file, gaps).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut probe = Probe::new(reader).options(ParseOptions::new().read_cover_art(false));
    if let Some(ft) = file_type {
        probe = probe.set_file_type(ft);
    }
    let tagged_file = probe.read().with_context(|| format!("Failed to read tags from {}", path.display()))?;

    let properties = tagged_file.properties();
    let duration = properties.duration();
    let sample_rate = properties.sample_rate().unwrap_or(44100);
    let bitrate = properties.overall_bitrate().unwrap_or(320);

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag.and_then(|t| t.title().map(|s| s.to_string())).unwrap_or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()
    });
    let artist = tag.and_then(|t| t.artist().map(|s| s.to_string())).unwrap_or_else(|| "Unknown Artist".to_string());
    let album = tag.and_then(|t| t.album().map(|s| s.to_string())).unwrap_or_else(|| "Unknown Album".to_string());
    let genre = tag.and_then(|t| t.genre().map(|s| s.to_string())).unwrap_or_else(|| "Unknown".to_string());
    let label = tag.and_then(|t| t.get_string(ItemKey::Publisher).map(|s| s.to_string())).unwrap_or_default();
    let remixer = tag.and_then(|t| t.get_string(ItemKey::Remixer).map(|s| s.to_string())).unwrap_or_default();
    let comment = tag.and_then(|t| t.get_string(ItemKey::Comment).map(|s| s.to_string())).unwrap_or_default();
    let year = tag.and_then(|t| t.get_string(ItemKey::Year).and_then(|s| s.parse::<u16>().ok())).unwrap_or(0);
    let disc_number = tag.and_then(|t| t.disk()).unwrap_or(0) as u16;
    let track_number = tag.and_then(|t| t.track()).unwrap_or(0);
    let file_size = std::fs::metadata(path)?.len();

    let raw_filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("unknown.mp3");
    let usb_path = format!("/Contents/{}/{}", sanitize_path_component(&artist), sanitize_path_component(raw_filename));

    Ok(Track {
        source_path: path.to_path_buf(),
        usb_path,
        title,
        artist,
        album,
        genre,
        label,
        remixer,
        comment,
        year,
        disc_number,
        track_number,
        tempo: 0,
        key: String::new(),
        duration_secs: duration.as_secs_f64(),
        sample_rate,
        bitrate,
        file_size,
        id,
        artwork: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::scanner::read_track_metadata;

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
        let dir = std::env::temp_dir().join(format!("ff-tagscan-{}", std::process::id()));
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
}
