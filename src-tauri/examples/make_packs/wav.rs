//! A minimal WAV writer: 44.1 kHz, mono, 16-bit PCM.

/// Encodes samples in `[-1, 1]` as a WAV file. Values outside are clamped.
pub fn encode(samples: &[f64]) -> Vec<u8> {
    let rate: u32 = 44_100;
    let channels: u16 = 1;
    let bits: u16 = 16;
    let block = channels * bits / 8;
    let data_len = (samples.len() * usize::from(block)) as u32;

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Decodes a source clip: RIFF/WAVE, PCM (format 1, or
/// `WAVE_FORMAT_EXTENSIBLE` with the PCM subformat), mono, 44.1 kHz, 16 or
/// 24-bit. Unknown chunks are skipped. Integers become `f64` by exact
/// division, so the result is the same on every OS.
pub fn decode(bytes: &[u8]) -> Result<Vec<f64>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(pos + 4) as usize;
        let body = pos + 8;
        if id == b"fmt " {
            if len < 16 || body + len > bytes.len() {
                return Err("bad fmt chunk".into());
            }
            let mut tag = u16_at(body);
            if tag == 0xFFFE {
                // WAVE_FORMAT_EXTENSIBLE: the subformat GUID starts with the tag.
                if len < 40 {
                    return Err("bad fmt chunk".into());
                }
                tag = u16_at(body + 24);
            }
            format = Some((tag, u16_at(body + 2), u32_at(body + 4), u16_at(body + 14)));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = format.ok_or("data before fmt")?;
            if tag != 1 {
                return Err(format!("format {tag} is not PCM"));
            }
            if channels != 1 {
                return Err(format!("{channels} channels; only mono is accepted"));
            }
            if rate != 44_100 {
                return Err(format!("{rate} Hz; only 44100 Hz is accepted"));
            }
            let width = match bits {
                16 => 2,
                24 => 3,
                _ => return Err(format!("{bits}-bit; only 16 or 24-bit is accepted")),
            };
            if body + len > bytes.len() || !len.is_multiple_of(width) {
                return Err("truncated data".into());
            }
            let data = &bytes[body..body + len];
            let samples = data
                .chunks_exact(width)
                .map(|c| {
                    if width == 2 {
                        f64::from(i16::from_le_bytes([c[0], c[1]])) / 32_768.0
                    } else {
                        // Sign-extend 24 bits by placing them in the top of an i32.
                        f64::from(i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) / 8_388_608.0
                    }
                })
                .collect();
            return Ok(samples);
        }
        // Chunks are padded to an even length.
        pos = body + len + (len & 1);
    }
    Err("no data chunk".into())
}
