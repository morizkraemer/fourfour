pub const HOT_CUE_PALETTE: [[u8; 3]; 16] = [
    [0xff, 0x2f, 0xa3],
    [0xff, 0x3b, 0x30],
    [0xff, 0x8a, 0x00],
    [0xff, 0xd6, 0x00],
    [0xa8, 0xe2, 0x00],
    [0x1f, 0xd3, 0x5a],
    [0x17, 0xd9, 0xa3],
    [0x1f, 0xd0, 0xe8],
    [0x33, 0xa1, 0xff],
    [0x3b, 0x5b, 0xff],
    [0x6a, 0x4c, 0xff],
    [0xa6, 0x4c, 0xff],
    [0xd8, 0x3c, 0xff],
    [0xff, 0x5c, 0x8a],
    [0xff, 0xa0, 0x7a],
    [0xe6, 0xe6, 0xe6],
];

/// Palette position `1..=16` for a cue colour written as `table:N` or `#rrggbb`
/// (nearest palette entry for other hex values). `None` for anything else.
pub fn table_index(color: &str) -> Option<u8> {
    if let Some(n) = color.strip_prefix("table:") {
        return n.parse::<u8>().ok().filter(|n| (1..=16).contains(n));
    }
    let hex = color.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    let (r, g, b) = (channel(0)?, channel(2)?, channel(4)?);
    let distance = |p: &[u8; 3]| {
        [(r, p[0]), (g, p[1]), (b, p[2])]
            .iter()
            .map(|&(a, b)| (a as i32 - b as i32).pow(2))
            .sum::<i32>()
    };
    HOT_CUE_PALETTE
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| distance(p))
        .map(|(i, _)| i as u8 + 1)
}

pub fn rgb(table_index: u8) -> [u8; 3] {
    HOT_CUE_PALETTE[table_index as usize - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_hex_maps_to_its_own_index() {
        for (i, p) in HOT_CUE_PALETTE.iter().enumerate() {
            let hex = format!("#{:02x}{:02x}{:02x}", p[0], p[1], p[2]);
            assert_eq!(table_index(&hex), Some(i as u8 + 1));
        }
    }

    #[test]
    fn table_names_and_near_hex_resolve() {
        assert_eq!(table_index("table:3"), Some(3));
        assert_eq!(table_index("table:0"), None);
        assert_eq!(table_index("table:17"), None);
        assert_eq!(table_index("#fe3a30"), Some(2));
        assert_eq!(table_index("red"), None);
    }
}
