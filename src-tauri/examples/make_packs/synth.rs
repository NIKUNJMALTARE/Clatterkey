//! Sound building blocks: a seeded random source, filters, damped
//! resonances and noise bursts.
//!
//! Every calculation here uses only `+`, `-`, `*`, `/`, `sqrt` and `round`
//! on `f64`. IEEE 754 defines those exactly, so the generated files are
//! byte-identical on every OS. The `std` functions `sin`, `cos`, `exp` and
//! `powf` call the platform maths library, whose last bit can differ between
//! macOS, Windows and Linux, so this file has its own; `mul_add` is not used
//! either, because a fused multiply-add rounds differently.

/// Output sample rate, Hz.
pub const RATE: f64 = 44_100.0;

// Constants are fixed values, the same on every platform.
use std::f64::consts::{LN_2 as LN2, TAU};

/// `e^x`, by range reduction to `|r| <= ln 2 / 2` and a fixed-length series.
pub fn exp(x: f64) -> f64 {
    if x < -700.0 {
        return 0.0;
    }
    let k = (x / LN2).round();
    let r = x - k * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..=24 {
        term = term * r / f64::from(n);
        sum += term;
    }
    sum * pow2(k as i64)
}

/// `2^k`, built from its bits.
fn pow2(k: i64) -> f64 {
    let k = k.clamp(-1022, 1023);
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// Reduces `x` to `[-pi, pi]`.
fn wrap(x: f64) -> f64 {
    x - (x / TAU).round() * TAU
}

/// `sin x`, by a fixed-length series after reduction to `[-pi, pi]`.
pub fn sin(x: f64) -> f64 {
    let x = wrap(x);
    let mut term = x;
    let mut sum = x;
    for n in 1..=30 {
        let n = f64::from(n);
        term = -term * x * x / ((2.0 * n) * (2.0 * n + 1.0));
        sum += term;
    }
    sum
}

/// `cos x`, as `sin`.
pub fn cos(x: f64) -> f64 {
    let x = wrap(x);
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..=30 {
        let n = f64::from(n);
        term = -term * x * x / ((2.0 * n - 1.0) * (2.0 * n));
        sum += term;
    }
    sum
}

/// Xorshift64: small, fast and the same everywhere.
pub struct Rng(u64);

impl Rng {
    /// Seeded from text (a pack and file name) with FNV-1a, so each file has
    /// its own fixed sequence.
    pub fn from_name(name: &str) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in name.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(if h == 0 { 0x9E37_79B9_7F4A_7C15 } else { h })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / 9_007_199_254_740_992.0
    }

    /// Uniform in `[-1, 1)`.
    pub fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }

    /// `x` changed by up to `±amount` (a fraction of `x`).
    pub fn vary(&mut self, x: f64, amount: f64) -> f64 {
        x * (1.0 + amount * self.signed())
    }
}

/// One damped resonance: frequency (Hz), decay time constant (s), gain.
#[derive(Clone, Copy)]
pub struct Mode(pub f64, pub f64, pub f64);

/// One impact: a burst of noise, filtered, played directly and through a
/// set of resonances.
#[derive(Clone)]
pub struct Strike {
    /// Start time, s.
    pub at: f64,
    /// Decay time constant of the noise burst, s.
    pub burst: f64,
    /// High-pass and low-pass corner on the burst, Hz; 0 for none.
    pub high_pass: f64,
    pub low_pass: f64,
    /// Level of the filtered burst heard directly.
    pub direct: f64,
    pub modes: Vec<Mode>,
    /// Overall level of this strike.
    pub level: f64,
}

/// One-pole low-pass coefficient for a corner frequency.
fn one_pole(corner: f64) -> f64 {
    1.0 - exp(-TAU * corner / RATE)
}

/// A two-pole resonator, scaled so its impulse response peaks near 1.
struct Resonator {
    b0: f64,
    a1: f64,
    a2: f64,
    y1: f64,
    y2: f64,
}

impl Resonator {
    fn new(freq: f64, decay: f64) -> Self {
        let freq = freq.min(RATE * 0.45);
        let w = TAU * freq / RATE;
        let r = exp(-1.0 / (decay * RATE));
        Self {
            b0: sin(w),
            a1: 2.0 * r * cos(w),
            a2: -r * r,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn step(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Renders strikes into a buffer `length` seconds long.
pub fn render(strikes: &[Strike], length: f64, rng: &mut Rng) -> Vec<f64> {
    let n = (length * RATE).round() as usize;
    let mut out = vec![0.0; n];
    for strike in strikes {
        let start = (strike.at * RATE).round() as usize;
        if start >= n {
            continue;
        }
        let fall = exp(-1.0 / (strike.burst * RATE));
        let hp = if strike.high_pass > 0.0 {
            Some(one_pole(strike.high_pass))
        } else {
            None
        };
        let lp = if strike.low_pass > 0.0 {
            Some(one_pole(strike.low_pass))
        } else {
            None
        };
        let mut resonators: Vec<(Resonator, f64)> = strike
            .modes
            .iter()
            .map(|m| (Resonator::new(m.0, m.1), m.2))
            .collect();
        let mut env = 1.0;
        let mut hp_state = 0.0;
        let mut lp_state = 0.0;
        for slot in out.iter_mut().skip(start) {
            let mut x = if env > 1e-7 { rng.signed() * env } else { 0.0 };
            env *= fall;
            if let Some(a) = hp {
                hp_state += a * (x - hp_state);
                x -= hp_state;
            }
            if let Some(a) = lp {
                lp_state += a * (x - lp_state);
                x = lp_state;
            }
            let mut y = strike.direct * x;
            for (res, gain) in &mut resonators {
                y += *gain * res.step(x);
            }
            *slot += strike.level * y;
        }
    }
    out
}

/// Largest absolute value.
pub fn peak(samples: &[f64]) -> f64 {
    samples
        .iter()
        .fold(0.0, |m, s| if s.abs() > m { s.abs() } else { m })
}

/// Root mean square.
pub fn rms(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f64).sqrt()
}

/// Multiplies every sample by `gain`.
pub fn scale(samples: &mut [f64], gain: f64) {
    for s in samples {
        *s *= gain;
    }
}

/// Removes any slow offset, cuts the tail once it has fallen 60 dB below
/// the peak, and fades the end so it does not click.
pub fn finish(mut samples: Vec<f64>) -> Vec<f64> {
    // A gentle high-pass at about 20 Hz removes DC from the low resonances.
    let a = one_pole(20.0);
    let mut low = 0.0;
    for s in &mut samples {
        low += a * (*s - low);
        *s -= low;
    }
    let p = peak(&samples);
    let floor = p * 0.001;
    let last = samples.iter().rposition(|s| s.abs() > floor).unwrap_or(0);
    let tail = (0.002 * RATE) as usize;
    let end = (last + tail).min(samples.len());
    samples.truncate(end);
    // A file that had not died away by its length gets a longer fade.
    let fade_secs = if end == last + tail { 0.003 } else { 0.06 };
    let fade = ((fade_secs * RATE) as usize).min(samples.len());
    let len = samples.len();
    for i in 0..fade {
        let k = i as f64 / fade as f64;
        samples[len - 1 - i] *= k * k;
    }
    samples
}

/// Adds `x` times `gain` into `out` from `at` seconds, growing `out` if `x`
/// runs past its end.
pub fn mix(out: &mut Vec<f64>, x: &[f64], at: f64, gain: f64) {
    let start = (at * RATE).round() as usize;
    if out.len() < start + x.len() {
        out.resize(start + x.len(), 0.0);
    }
    for (slot, s) in out.iter_mut().skip(start).zip(x) {
        *slot += gain * s;
    }
}
