use std::fmt::Write;

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

pub fn start(out: &mut String, depth: usize, name: &str, attrs: &[(&str, String)], self_closing: bool) {
    let _ = write!(out, "{:indent$}<{name}", "", indent = depth * 2);
    for (key, value) in attrs {
        let _ = write!(out, " {key}=\"{}\"", escape(value));
    }
    out.push_str(if self_closing { "/>\n" } else { ">\n" });
}

pub fn end(out: &mut String, depth: usize, name: &str) {
    let _ = writeln!(out, "{:indent$}</{name}>", "", indent = depth * 2);
}

pub fn seconds(ms: u32) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// `#rrggbb` or a palette name (`table:N`) as red, green, blue.
pub fn rgb(color: &str) -> Option<[u8; 3]> {
    if let Some(hex) = color.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii()) {
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        return Some([channel(0)?, channel(2)?, channel(4)?]);
    }
    pioneer_usb_writer::cue_color::table_index(color).map(pioneer_usb_writer::cue_color::rgb)
}

pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub fn extension(path: &str) -> String {
    file_name(path).rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_entities_and_drops_invalid_control_characters() {
        assert_eq!(escape("Tom & \"Jerry\" <1>'\u{1}\n"), "Tom &amp; &quot;Jerry&quot; &lt;1&gt;&apos;&#10;");
    }

    #[test]
    fn colours_parse_from_hex_and_palette() {
        assert_eq!(rgb("#28e214"), Some([0x28, 0xe2, 0x14]));
        assert!(rgb("table:3").is_some());
        assert_eq!(rgb("red"), None);
    }
}
