//! A minimal `.ico` writer: a directory followed by the images, each
//! stored as a PNG, which Windows has read since Vista.

/// Encodes square images, given as (pixels along a side, PNG bytes).
pub fn encode(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    // Reserved, type 1 (icon), the number of images.
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * images.len();
    for (size, png) in images {
        // A side of 256 is written as 0.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        // One colour plane, 32 bits per pixel.
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}
