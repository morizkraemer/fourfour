pub struct Variant {
    pub label: String,
    pub bytes: Vec<u8>,
}

fn variant(label: &str, bytes: Vec<u8>) -> Variant {
    Variant { label: label.to_string(), bytes }
}

fn synchsafe(n: u32) -> [u8; 4] {
    [(n >> 21 & 0x7f) as u8, (n >> 14 & 0x7f) as u8, (n >> 7 & 0x7f) as u8, (n & 0x7f) as u8]
}

fn id3_tag(version: u8, flags: u8, claimed: u32, body: &[u8]) -> Vec<u8> {
    let mut out = vec![b'I', b'D', b'3', version, 0, flags];
    out.extend_from_slice(&synchsafe(claimed));
    out.extend_from_slice(body);
    out
}

fn id3_frame(version: u8, id: &[u8; 4], claimed: u32, body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    if version == 4 {
        out.extend_from_slice(&synchsafe(claimed));
    } else {
        out.extend_from_slice(&claimed.to_be_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(body);
    out
}

fn jpeg(len: usize) -> Vec<u8> {
    let mut data = vec![0u8; len];
    data[..4].copy_from_slice(&[0xff, 0xd8, 0xff, 0xe0]);
    data
}

fn apic_body(picture: &[u8]) -> Vec<u8> {
    let mut body = vec![0u8];
    body.extend_from_slice(b"image/jpeg\0");
    body.extend_from_slice(&[3, 0]);
    body.extend_from_slice(picture);
    body
}

fn text_frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    id3_frame(4, id, body.len() as u32, body)
}

fn strip_id3(bytes: &[u8]) -> &[u8] {
    if bytes.len() >= 10 && &bytes[..3] == b"ID3" {
        let size = (bytes[6] as usize) << 21 | (bytes[7] as usize) << 14 | (bytes[8] as usize) << 7 | bytes[9] as usize;
        &bytes[(10 + size).min(bytes.len())..]
    } else {
        bytes
    }
}

fn find(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    bytes.windows(needle.len()).position(|w| w == needle)
}

fn proper_tag(frames: &[u8]) -> Vec<u8> {
    id3_tag(4, 0, frames.len() as u32, frames)
}

fn id3_hostile(audio: &[u8]) -> Vec<(String, Vec<u8>)> {
    let small = text_frame(b"TIT2", b"\x03fuzz");
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, claim) in [("claim-50MB", 50u32 << 20), ("claim-250MB", 0x0fff_ffff), ("claim-4GB-v3", 0)] {
        let tag = if claim == 0 { id3_tag(3, 0, 0x0fff_ffff, &id3_frame(3, b"APIC", 0xffff_ffff, &small)) } else { id3_tag(4, 0, claim, &small) };
        out.push((name.into(), tag));
    }
    let real_apic = text_frame_len(b"APIC", &apic_body(&jpeg(20 << 20)));
    out.push(("apic-20MB-real".into(), proper_tag(&real_apic)));
    out.push(("apic-claims-250MB".into(), id3_tag(4, 0, 0x0fff_ffff, &id3_frame(4, b"APIC", 0x0fff_ff00, &apic_body(&jpeg(1000))))));
    out.push(("apic-v3-frame-4GB".into(), id3_tag(3, 0, 2000, &id3_frame(3, b"APIC", 0xffff_fff0, &apic_body(&jpeg(1000))))));
    let txxx: Vec<u8> = (0..100_000).flat_map(|_| text_frame(b"TXXX", b"\x03a\0b")).collect();
    out.push(("frames-100k-txxx".into(), proper_tag(&txxx)));
    let empty: Vec<u8> = (0..200_000).flat_map(|_| id3_frame(4, b"TIT2", 1, b"\x03")).collect();
    out.push(("frames-200k-tiny".into(), proper_tag(&empty)));
    let bad16: Vec<u8> = [
        text_frame(b"TIT2", &[1, 0xff, 0xfe, 0x00, 0xd8, 0x41, 0x00, 0x42]),
        text_frame(b"TPE1", &[1, 0x41, 0x00, 0x00, 0xdc]),
        text_frame(b"TALB", &[2, 0x00, 0xd8, 0x00, 0xd8, 0x00]),
        text_frame(b"TCON", &[1, 0xfe, 0xff, 0xd8, 0x00, 0xdc, 0x00, 0x00]),
        text_frame(b"COMM", &[1, b'e', b'n', b'g', 0xff, 0xfe, 0x01, 0xd8, 0x00, 0x00, 0x00]),
        text_frame(b"TXXX", &[1, 0xff]),
    ]
    .concat();
    out.push(("utf16-invalid".into(), proper_tag(&bad16)));
    let mut ext = vec![0x7f, 0xff, 0xff, 0xff, 1, 0];
    ext.extend_from_slice(&small);
    out.push(("ext-header-huge".into(), id3_tag(4, 0x40, 0x0fff_ffff, &ext)));
    let unsync: Vec<u8> = std::iter::repeat([0xffu8, 0x00]).take(2 << 20).flatten().collect();
    out.push(("unsync-4MB".into(), id3_tag(4, 0x80, unsync.len() as u32 & 0x0fff_ffff, &unsync)));
    let mut v22 = b"PIC\xff\xff\xff".to_vec();
    v22.extend_from_slice(&[0; 40]);
    out.push(("v22-pic-16MB".into(), id3_tag(2, 0, 50, &v22)));
    out.push(("footer-flag".into(), id3_tag(4, 0x10, 30, &small)));
    let stacked: Vec<u8> = (0..5000).flat_map(|_| proper_tag(&small)).collect();
    out.push(("stacked-5000".into(), stacked));
    out.into_iter().map(|(n, mut tag)| { tag.extend_from_slice(audio); (n, tag) }).collect()
}

fn text_frame_len(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    id3_frame(4, id, body.len() as u32 & 0x0fff_ffff, body)
}

pub fn mp3_variants(base: &[u8]) -> Vec<Variant> {
    id3_hostile(strip_id3(base)).into_iter().map(|(n, b)| variant(&format!("mp3-{n}"), b)).collect()
}

fn flac_block(kind: u8, last: bool, claimed: u32, body: &[u8]) -> Vec<u8> {
    let mut out = vec![kind | if last { 0x80 } else { 0 }];
    out.extend_from_slice(&claimed.to_be_bytes()[1..]);
    out.extend_from_slice(body);
    out
}

fn flac_picture(mime_len: u32, data_len: u32, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&3u32.to_be_bytes());
    body.extend_from_slice(&mime_len.to_be_bytes());
    body.extend_from_slice(b"image/jpeg");
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&[0; 16]);
    body.extend_from_slice(&data_len.to_be_bytes());
    body.extend_from_slice(data);
    body
}

fn vorbis_comment(vendor_len: u32, count: u32, comments: &[u8]) -> Vec<u8> {
    let mut body = vendor_len.to_le_bytes().to_vec();
    body.extend_from_slice(b"fuzz");
    body.extend_from_slice(&count.to_le_bytes());
    body.extend_from_slice(comments);
    body
}

fn big_picture() -> Vec<u8> {
    let data_len = 0x00ff_ffff - 42;
    flac_picture(10, data_len as u32, &vec![0; data_len])
}

pub fn flac_variants(base: &[u8]) -> Vec<Variant> {
    let mut pos = 4;
    let mut last_header = 4;
    while pos + 4 <= base.len() {
        last_header = pos;
        let header = base[pos];
        let len = (base[pos + 1] as usize) << 16 | (base[pos + 2] as usize) << 8 | base[pos + 3] as usize;
        pos += 4 + len;
        if header & 0x80 != 0 {
            break;
        }
    }
    let pos = pos.min(base.len());
    let mut meta = base[4..pos].to_vec();
    if last_header >= 4 && last_header - 4 < meta.len() {
        meta[last_header - 4] &= 0x7f;
    }
    let frames = &base[pos..];
    let build = |blocks: Vec<Vec<u8>>| {
        let mut out = b"fLaC".to_vec();
        out.extend_from_slice(&meta);
        out.extend(blocks.concat());
        out.extend_from_slice(frames);
        out
    };
    let comments: Vec<u8> = (0..1_000_000).flat_map(|_| [3u8, 0, 0, 0, b'A', b'=', b'B']).collect();
    let many: Vec<Vec<u8>> = (0..200_000).map(|_| flac_block(2, false, 4, b"fuzz")).collect();
    let mut many_blocks = many;
    many_blocks.push(flac_block(1, true, 0, b""));
    let id3_prefix = |tag: Vec<u8>| [tag, base.to_vec()].concat();
    vec![
        variant("flac-vc-count-max", build(vec![flac_block(4, true, 12, &vorbis_comment(4, 0xffff_ffff, b""))])),
        variant("flac-vc-vendor-max", build(vec![flac_block(4, true, 12, &vorbis_comment(0xffff_ffff, 0, b""))])),
        variant("flac-vc-1M-comments", build(vec![flac_block(4, true, comments.len() as u32 + 12, &vorbis_comment(4, 1_000_000, &comments))])),
        variant("flac-vc-comment-len-max", build(vec![flac_block(4, true, 24, &vorbis_comment(4, 1, &[0xff, 0xff, 0xff, 0xff, b'A', b'=', b'B', b'x']))])),
        variant("flac-picture-claims-4GB", build(vec![flac_block(6, true, 60, &flac_picture(0xffff_ffff, 0xffff_ffff, b"xx"))])),
        variant("flac-picture-data-4GB", build(vec![flac_block(6, true, 60, &flac_picture(10, 0xffff_ff00, b"xx"))])),
        variant("flac-picture-2x16MB-real", build(vec![flac_block(6, false, 0x00ff_ffff, &big_picture()), flac_block(6, true, 0x00ff_ffff, &big_picture())])),
        variant("flac-200k-small-blocks", build(many_blocks)),
        variant("flac-padding-16MB", build(vec![flac_block(1, true, 0x00ff_ffff, &vec![0; 0x00ff_ffff])])),
        variant("flac-id3-prefix-claim-250MB", id3_prefix(id3_tag(4, 0, 0x0fff_ffff, &text_frame(b"TIT2", b"\x03x")))),
        variant("flac-id3-prefix-apic-20MB", id3_prefix(proper_tag(&text_frame_len(b"APIC", &apic_body(&jpeg(20 << 20)))))),
    ]
}

fn riff_chunk(id: &[u8; 4], claimed: u32, body: &[u8], big_endian: bool) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&if big_endian { claimed.to_be_bytes() } else { claimed.to_le_bytes() });
    out.extend_from_slice(body);
    out
}

fn chunked_variants(prefix: &str, base: &[u8], big_endian: bool, id3_id: &[u8; 4]) -> Vec<Variant> {
    let head = &base[..12.min(base.len())];
    let rest = &base[12.min(base.len())..];
    let with = |chunks: Vec<u8>| [head, &chunks, rest].concat();
    let small_tag = proper_tag(&text_frame(b"TIT2", b"\x03x"));
    let apic20 = proper_tag(&text_frame_len(b"APIC", &apic_body(&jpeg(20 << 20))));
    let tiny: Vec<u8> = (0..100_000).flat_map(|_| riff_chunk(b"junk", 0, b"", big_endian)).collect();
    let info: Vec<u8> = (0..100_000).flat_map(|_| riff_chunk(b"INAM", 1, b"a\0", big_endian)).collect();
    let stacked: Vec<u8> = (0..5000).flat_map(|_| proper_tag(&small_tag)).collect();
    let mut v = vec![
        variant("id3-claims-500MB", with(riff_chunk(id3_id, 500 << 20, &id3_tag(4, 0, 500 << 20, &small_tag), big_endian))),
        variant("id3-apic-20MB-real", with(riff_chunk(id3_id, apic20.len() as u32, &apic20, big_endian))),
        variant("id3-stacked-5000", with(riff_chunk(id3_id, stacked.len() as u32, &stacked, big_endian))),
        variant("chunk-size-max", with(riff_chunk(b"junk", 0xffff_ffff, b"abcd", big_endian))),
        variant("list-size-max", with(riff_chunk(b"LIST", 0xffff_ffff, b"INFOINAM\x01\0\0\0a\0", big_endian))),
        variant("100k-empty-chunks", with(tiny)),
        variant("100k-info-subchunks", with(riff_chunk(b"LIST", info.len() as u32 + 4, &[b"INFO".to_vec(), info].concat(), big_endian))),
        variant("chunk-odd-size-max", with(riff_chunk(b"NAME", 0x7fff_ffff, b"x", big_endian))),
    ];
    let mut zero_size = base.to_vec();
    let mut max_size = base.to_vec();
    if zero_size.len() >= 8 {
        zero_size[4..8].fill(0);
        max_size[4..8].fill(0xff);
    }
    v.push(variant("container-size-0", zero_size));
    v.push(variant("container-size-max", max_size));
    if big_endian {
        v.push(variant("comt-65535-comments", with(riff_chunk(b"COMT", 2, &[0xff, 0xff], true))));
        v.push(variant("mark-65535-markers", with(riff_chunk(b"MARK", 2, &[0xff, 0xff], true))));
    } else {
        v.push(variant("bext-huge", with(riff_chunk(b"bext", 0x00ff_ffff, &[0u8; 602], false))));
        v.push(variant("cue-count-max", with(riff_chunk(b"cue ", 4, &0xffff_ffffu32.to_le_bytes(), false))));
    }
    v.into_iter().map(|x| variant(&format!("{prefix}-{}", x.label), x.bytes)).collect()
}

pub fn wav_variants(base: &[u8]) -> Vec<Variant> {
    chunked_variants("wav", base, false, b"id3 ")
}

pub fn aiff_variants(base: &[u8]) -> Vec<Variant> {
    chunked_variants("aiff", base, true, b"ID3 ")
}

fn atom(kind: &[u8; 4], size: u32, body: &[u8]) -> Vec<u8> {
    let mut out = size.to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn sized(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    atom(kind, body.len() as u32 + 8, body)
}

fn meta_with(ilst_items: &[u8]) -> Vec<u8> {
    let hdlr = sized(b"hdlr", &[0u8; 24]);
    sized(b"udta", &sized(b"meta", &[vec![0u8; 4], hdlr, sized(b"ilst", ilst_items)].concat()))
}

fn data_atom(size: u32, payload: &[u8]) -> Vec<u8> {
    atom(b"data", size, &[&[0, 0, 0, 13, 0, 0, 0, 0][..], payload].concat())
}

pub fn m4a_variants(base: &[u8]) -> Vec<Variant> {
    let mut pos = 0;
    let mut moov: Option<usize> = None;
    while pos + 8 <= base.len() {
        let size = u32::from_be_bytes(base[pos..pos + 4].try_into().unwrap()) as usize;
        if &base[pos + 4..pos + 8] == b"moov" {
            moov = Some(pos);
        }
        if size < 8 {
            break;
        }
        pos += size;
    }
    let append = |extra: Vec<u8>| -> Vec<u8> {
        let mut out = base.to_vec();
        match moov {
            Some(at) => {
                let old = u32::from_be_bytes(out[at..at + 4].try_into().unwrap()) as usize;
                let end = at + old;
                out.splice(end..end, extra.iter().copied());
                let new = (old + extra.len()) as u32;
                out[at..at + 4].copy_from_slice(&new.to_be_bytes());
            }
            None => out.extend_from_slice(&extra),
        }
        out
    };
    let covr = |data: Vec<u8>| sized(b"covr", &data);
    let nested: Vec<u8> = {
        let mut inner = Vec::new();
        for _ in 0..10_000 {
            inner = sized(b"meta", &inner);
            if inner.len() > 200_000 {
                break;
            }
        }
        inner
    };
    let flat_deep: Vec<u8> = (0..20_000).flat_map(|i| atom(b"meta", 8 + (20_000 - i - 1) * 8, b"")).collect();
    let empties: Vec<u8> = (0..1_000_000).flat_map(|_| atom(b"free", 8, b"")).collect();
    let items: Vec<u8> = (0..200_000).flat_map(|_| sized(b"\xa9nam", &data_atom(21, b"abcde"))).collect();
    let patch = |name: &[u8; 4], offset: usize, value: u32| -> Vec<u8> {
        let mut out = base.to_vec();
        if let Some(p) = find(&out, name) {
            if p + offset + 4 <= out.len() {
                out[p + offset..p + offset + 4].copy_from_slice(&value.to_be_bytes());
            }
        }
        out
    };
    let mut v = vec![
        variant("covr-claims-500MB", append(meta_with(&covr(data_atom(500 << 20, b"xx"))))),
        variant("covr-20MB-real", append(meta_with(&covr(data_atom(20 << 20, &vec![0xaa; (20 << 20) - 16]))))),
        variant("covr-largesize-2^63", append(meta_with(&covr([atom(b"data", 1, &(1u64 << 63).to_be_bytes()), vec![0; 8]].concat())))),
        variant("covr-size-0-to-eof", append(meta_with(&covr(atom(b"data", 0, b"xxxx"))))),
        variant("covr-size-below-8", append(meta_with(&covr(atom(b"data", 4, b"xxxxxxxx"))))),
        variant("meta-nested-self", append(sized(b"udta", &nested))),
        variant("meta-flat-20k-deep", append(sized(b"udta", &flat_deep))),
        variant("udta-1M-empty-atoms", append(sized(b"udta", &empties))),
        variant("ilst-200k-items", append(meta_with(&items))),
        variant("moov-size-max", patch_moov_size(base, moov, 0xffff_ffff)),
        variant("moov-size-1", patch_moov_size(base, moov, 1)),
        variant("stsz-count-max", patch(b"stsz", 12, 0xffff_ffff)),
        variant("stts-count-max", patch(b"stts", 8, 0xffff_ffff)),
        variant("stco-count-max", patch(b"stco", 8, 0xffff_ffff)),
        variant("stsc-count-max", patch(b"stsc", 8, 0xffff_ffff)),
        variant("mdhd-timescale-0", patch(b"mdhd", 16, 0)),
        variant("mvhd-timescale-0", patch(b"mvhd", 16, 0)),
        variant("stsz-sample-size-max", patch(b"stsz", 8, 0xffff_ffff)),
    ];
    for x in &mut v {
        x.label = format!("m4a-{}", x.label);
    }
    v
}

fn patch_moov_size(base: &[u8], moov: Option<usize>, value: u32) -> Vec<u8> {
    let mut out = base.to_vec();
    if let Some(at) = moov {
        out[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }
    out
}

fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04c1_1db7 } else { crc << 1 };
        }
    }
    crc
}

fn ogg_patch(base: &[u8], edit: impl Fn(&mut [u8], usize)) -> Vec<u8> {
    let mut out = base.to_vec();
    let Some(tag) = find(&out, b"\x03vorbis") else { return out };
    edit(&mut out, tag);
    let Some(page) = out[..tag].windows(4).rposition(|w| w == b"OggS") else { return out };
    let segments = out[page + 26] as usize;
    let body: usize = out[page + 27..page + 27 + segments].iter().map(|&s| s as usize).sum();
    let end = (page + 27 + segments + body).min(out.len());
    out[page + 22..page + 26].fill(0);
    let crc = ogg_crc(&out[page..end]);
    out[page + 22..page + 26].copy_from_slice(&crc.to_le_bytes());
    out
}

pub fn ogg_variants(base: &[u8]) -> Vec<Variant> {
    let vendor_len_at = |tag: usize| tag + 7;
    let count_at = |out: &[u8], tag: usize| {
        let vendor = u32::from_le_bytes(out[vendor_len_at(tag)..vendor_len_at(tag) + 4].try_into().unwrap()) as usize;
        tag + 11 + vendor
    };
    let set = |at: usize, out: &mut [u8], value: u32| {
        if at + 4 <= out.len() {
            out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    };
    vec![
        variant("ogg-vc-count-max", ogg_patch(base, |o, t| { let at = count_at(o, t); set(at, o, 0xffff_ffff) })),
        variant("ogg-vc-count-1M", ogg_patch(base, |o, t| { let at = count_at(o, t); set(at, o, 1_000_000) })),
        variant("ogg-vc-vendor-max", ogg_patch(base, |o, t| set(vendor_len_at(t), o, 0xffff_ffff))),
        variant("ogg-vc-first-comment-max", ogg_patch(base, |o, t| { let at = count_at(o, t) + 4; set(at, o, 0xffff_ffff) })),
        variant("ogg-vc-count-1", ogg_patch(base, |o, t| { let at = count_at(o, t); set(at, o, 1) })),
        variant("ogg-vc-vendor-0", ogg_patch(base, |o, t| set(vendor_len_at(t), o, 0))),
    ]
}
