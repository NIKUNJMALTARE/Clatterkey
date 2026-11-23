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
