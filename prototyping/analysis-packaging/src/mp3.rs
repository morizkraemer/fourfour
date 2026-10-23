// Frame scan and frame-map rule ported from cyanbox src/audio/mp3.c.
// Copyright (c) 2026, Lynne. BSD-2-Clause, see NOTICE.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAP_SETTLED: usize = 4;
const MAP_FRAMES: usize = MAP_SETTLED + 2;
const SCAN_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Header {
    version: u8,
    layer: u8,
    rate_index: u8,
    mode: u8,
    rate: u32,
    samples: u32,
    length: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Mp3Timeline {
    pub sample_rate: u32,
    pub frame_samples: u32,
    pub lame_delay: Option<u32>,
    pub header_frame: bool,
    pub header_counts: bool,
    pub encoder: Option<[u8; 9]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DelayRule {
    None,
    Lame,
    LameAndHeaderFrame,
    HeaderFrame,
    Encoder,
    EncoderAndHeaderFrame,
    EncoderHeaderLame3100Early,
}

impl DelayRule {
    pub fn parse(s: &str) -> Option<DelayRule> {
        match s {
            "none" => Some(DelayRule::None),
            "lame" => Some(DelayRule::Lame),
            "lame+header" => Some(DelayRule::LameAndHeaderFrame),
            "header" => Some(DelayRule::HeaderFrame),
            "enc" => Some(DelayRule::Encoder),
            "enc+header" => Some(DelayRule::EncoderAndHeaderFrame),
            "enc+header+l3100" => Some(DelayRule::EncoderHeaderLame3100Early),
            _ => None,
        }
    }
}

const BITRATES: [[u32; 16]; 2] = [
    [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0],
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
];

fn parse_header(d: &[u8], pos: usize) -> Option<Header> {
    if pos + 4 > d.len() || d[pos] != 0xff || d[pos + 1] & 0xe0 != 0xe0 {
        return None;
    }
    let version = (d[pos + 1] >> 3) & 3;
    let layer = (d[pos + 1] >> 1) & 3;
    let bitrate_index = (d[pos + 2] >> 4) as usize;
    let rate_index = (d[pos + 2] >> 2) & 3;
    let padding = ((d[pos + 2] >> 1) & 1) as usize;
    let mode = (d[pos + 3] >> 6) & 3;
    if version == 1 || layer != 1 || rate_index == 3 || bitrate_index == 0 || bitrate_index == 15 {
        return None;
    }
    let mpeg1 = version == 3;
    let rate = match (version, rate_index) {
        (3, 0) => 44100,
        (3, 1) => 48000,
        (3, 2) => 32000,
        (2, 0) => 22050,
        (2, 1) => 24000,
        (2, 2) => 16000,
        (0, 0) => 11025,
        (0, 1) => 12000,
        (0, 2) => 8000,
        _ => return None,
    };
    let kbps = BITRATES[if mpeg1 { 0 } else { 1 }][bitrate_index];
    let samples = if mpeg1 { 1152 } else { 576 };
    let length = (samples as usize / 8 * kbps as usize * 1000) / rate as usize + padding;
    Some(Header { version, layer, rate_index, mode, rate, samples, length })
}

fn side_info_size(h: &Header) -> usize {
    match (h.version == 3, h.mode == 3) {
        (true, true) => 17,
        (true, false) => 32,
        (false, true) => 9,
        (false, false) => 17,
    }
}

fn same_shape(a: &Header, b: &Header) -> bool {
    a.version == b.version && a.layer == b.layer && a.rate_index == b.rate_index && a.mode == b.mode
}

fn id3v2_size(head: &[u8]) -> usize {
    if head.len() >= 10 && &head[..3] == b"ID3" {
        let s = ((head[6] & 0x7f) as usize) << 21 | ((head[7] & 0x7f) as usize) << 14 | ((head[8] & 0x7f) as usize) << 7 | (head[9] & 0x7f) as usize;
        10 + s + if head[5] & 0x10 != 0 { 10 } else { 0 }
    } else {
        0
    }
}

pub fn scan(path: &Path) -> Option<Mp3Timeline> {
    if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("mp3")) {
        return None;
    }
    let mut f = File::open(path).ok()?;
    let mut head = [0u8; 10];
    f.read_exact(&mut head).ok()?;
    let start_at = id3v2_size(&head) as u64;
    f.seek(SeekFrom::Start(start_at)).ok()?;
    let mut d = Vec::with_capacity(SCAN_BYTES);
    f.take(SCAN_BYTES as u64).read_to_end(&mut d).ok()?;

    let mut start = 0;
    let first = loop {
        if start + 4 > d.len().min(4096) {
            return None;
        }
        if let Some(h) = parse_header(&d, start) {
            if let Some(n) = parse_header(&d, start + h.length) {
                if n.version == h.version && n.layer == h.layer && n.rate_index == h.rate_index {
                    break h;
                }
            }
        }
        start += 1;
    };
    let tag_pos = start + 4 + side_info_size(&first);
    let tag_at = |pos: usize, id: &[u8; 4]| d.len() >= pos + 4 && &d[pos..pos + 4] == id;
    let xing = tag_at(tag_pos, b"Xing");
    let vbri = tag_at(start + 36, b"VBRI");
    let header_frame = xing || vbri || tag_at(tag_pos, b"Info");

    let mut map: Vec<Header> = Vec::new();
    let mut pos = start;
    while map.len() < MAP_FRAMES {
        let Some(h) = parse_header(&d, pos) else { break };
        pos += h.length;
        map.push(h);
    }
    let mut map_start = 0;
    for i in 0..map.len().saturating_sub(1) {
        let held = i + 1 - map_start;
        if !same_shape(&map[i], &map[i + 1]) && held < MAP_SETTLED {
            map_start = i + 1;
        }
    }

    let mut encoder = None;
    let lame_delay = if header_frame {
        let mut ext = tag_pos + 8;
        if d.len() >= ext {
            let flags = u32::from_be_bytes([d[tag_pos + 4], d[tag_pos + 5], d[tag_pos + 6], d[tag_pos + 7]]);
            if flags & 1 != 0 { ext += 4; }
            if flags & 2 != 0 { ext += 4; }
            if flags & 4 != 0 { ext += 100; }
            if flags & 8 != 0 { ext += 4; }
        }
        if ext + 24 <= d.len() && d[ext..ext + 4].iter().all(|c| c.is_ascii_alphanumeric()) {
            encoder = Some(d[ext..ext + 9].try_into().unwrap());
            Some(((d[ext + 21] as u32) << 4) | ((d[ext + 22] >> 4) as u32))
        } else {
            None
        }
    } else {
        None
    };
    Some(Mp3Timeline {
        sample_rate: first.rate,
        frame_samples: first.samples,
        lame_delay,
        header_frame,
        header_counts: header_frame && map_start == 0,
        encoder,
    })
}

pub fn reference_delay_s(path: &Path, rule: DelayRule) -> f64 {
    if rule == DelayRule::None {
        return 0.0;
    }
    let Some(t) = scan(path) else { return 0.0 };
    let sr = t.sample_rate as f64;
    let lame = t.lame_delay.map_or(0.0, |d| (d + 529) as f64 / sr);
    let enc = t.lame_delay.map_or(0.0, |d| d as f64 / sr);
    let header = if t.header_counts { t.frame_samples as f64 / sr } else { 0.0 };
    match rule {
        DelayRule::None => 0.0,
        DelayRule::Lame => lame,
        DelayRule::LameAndHeaderFrame => lame + header,
        DelayRule::HeaderFrame => header,
        DelayRule::Encoder => enc,
        DelayRule::EncoderAndHeaderFrame => enc + header,
        DelayRule::EncoderHeaderLame3100Early => {
            let early = t.encoder.is_some_and(|e| e.starts_with(b"Lame3.100"));
            enc + header - if early { t.frame_samples as f64 / sr } else { 0.0 }
        }
    }
}
