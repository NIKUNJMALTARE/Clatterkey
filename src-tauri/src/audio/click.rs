//! The built-in click: a placeholder made for this project and generated in
//! code, so the app needs no audio file and no decoder to make a sound.

use std::f32::consts::TAU;

use super::Sample;

const SAMPLE_RATE: u32 = 48_000;
/// 60 ms.
const FRAMES: usize = 2_880;
/// The last 5 ms fade to silence so the click does not end on a step.
const FADE_FRAMES: usize = 240;
/// Leaves room for four clicks at the same instant without clipping.
const PEAK: f32 = 0.2;

/// Generates the click: a burst of noise for the strike and two damped tones
/// for the body. It is the same on every call.
pub fn builtin_click() -> Sample {
    let mut noise = Noise(0x2545_F491);
    let mut data: Vec<f32> = (0..FRAMES)
        .map(|frame| {
            let t = frame as f32 / SAMPLE_RATE as f32;
            let strike = noise.next() * (-t / 0.004).exp();
            let ring = 0.6 * (TAU * 3_200.0 * t).sin() * (-t / 0.006).exp();
            let body = 0.5 * (TAU * 1_100.0 * t).sin() * (-t / 0.015).exp();
            let remaining = FRAMES - 1 - frame;
            let fade = (remaining as f32 / FADE_FRAMES as f32).min(1.0);
            (strike + ring + body) * fade
        })
        .collect();

    let peak = data.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
    if peak > 0.0 {
        let gain = PEAK / peak;
        for sample in &mut data {
            *sample *= gain;
        }
    }
    Sample::new(1, SAMPLE_RATE, data)
}

/// A fixed-seed pseudo-random generator (xorshift), giving the same noise on
/// every run without a dependency.
struct Noise(u32);

impl Noise {
    /// The next value, between -1.0 and 1.0.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}
