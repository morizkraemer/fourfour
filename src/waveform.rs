use crate::models::WaveformPreview;

/// Generate a 400-byte monochrome waveform preview (PWAV format).
///
/// Each byte encodes:
/// - bits 0-4: height (0-31)
/// - bits 5-7: whiteness/intensity (0-7, higher = brighter)
pub fn generate_preview(samples: &[f32]) -> WaveformPreview {
    let mut data = [0u8; 400];

    if samples.is_empty() {
        return WaveformPreview { data };
    }

    let chunk_size = samples.len() / 400;
    if chunk_size == 0 {
        return WaveformPreview { data };
    }

    // Compute RMS for each of 400 segments
    let mut rms_values = [0.0f32; 400];
    let mut max_rms = 0.0f32;

    for (i, chunk) in samples.chunks(chunk_size).take(400).enumerate() {
        let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
        let rms = (sum_sq / chunk.len() as f32).sqrt();
        rms_values[i] = rms;
        if rms > max_rms {
            max_rms = rms;
        }
    }

    // Normalize and encode
    if max_rms > 0.0 {
        for (i, &rms) in rms_values.iter().enumerate() {
            let normalized = rms / max_rms;
            let height = (normalized * 31.0) as u8; // 5 bits: 0-31
            let whiteness = (normalized * 7.0) as u8; // 3 bits: 0-7
            data[i] = (whiteness << 5) | (height & 0x1F);
        }
    }

    WaveformPreview { data }
}
