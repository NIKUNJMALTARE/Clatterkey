//! Tests for the `rodio` back end that need no sound device: the mixer
//! sink is driven through a deviceless mixer and its output read directly.

use std::num::NonZero;

use rodio::mixer::{MixerSource, mixer};

use super::{MixerSink, buffer_frames};
use crate::audio::{Sample, Sink, builtin_click};

const RATE: u32 = 48_000;

fn mono_mixer(rate: u32) -> (MixerSink, MixerSource) {
    let (mixer, source) = mixer(NonZero::new(1).unwrap(), NonZero::new(rate).unwrap());
    (MixerSink::new(mixer), source)
}

fn pull(source: &mut MixerSource, count: usize) -> Vec<f32> {
    source.by_ref().take(count).collect()
}

/// A ramp of distinct values, so a shifted or restarted copy is noticed.
fn ramp(len: usize) -> Sample {
    Sample::new(
        1,
        RATE,
        (1..=len).map(|i| i as f32 * 0.0005).collect::<Vec<f32>>(),
    )
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn ac4_a_second_click_does_not_stop_or_restart_the_first() {
    let (sink, mut source) = mono_mixer(RATE);
    let first = ramp(100);
    sink.play(&first);
    let mut out = pull(&mut source, 10);

    // A silent second click: any change in the output is damage to the first.
    sink.play(&Sample::new(1, RATE, vec![0.0; 5]));
    out.extend(source.by_ref());

    assert_eq!(out.len(), 100);
    assert_eq!(out, first.samples());
}

#[test]
fn ac4_ten_clicks_at_once_are_ten_voices() {
    let (sink, mut source) = mono_mixer(RATE);
    let click = ramp(50);
    for _ in 0..10 {
        sink.play(&click);
    }
    let out: Vec<f32> = source.by_ref().collect();
    assert_eq!(out.len(), 50);
    for (o, s) in out.iter().zip(click.samples()) {
        assert!(approx(*o, s * 10.0), "{o} vs {}", s * 10.0);
    }
}

#[test]
fn ac5_overlapping_clicks_are_summed_and_run_to_the_later_end() {
    let (sink, mut source) = mono_mixer(RATE);
    let click = ramp(100);
    let gap = 10;

    sink.play(&click);
    let mut out = pull(&mut source, gap);
    sink.play(&click);
    out.extend(source.by_ref());

    assert_eq!(out.len(), 100 + gap);
    let s = click.samples();
    for (i, value) in out.iter().enumerate() {
        let first = s.get(i).copied().unwrap_or(0.0);
        let second = if i >= gap {
            s.get(i - gap).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        assert!(approx(*value, first + second), "frame {i}: {value}");
    }
}

#[test]
fn four_builtin_clicks_at_the_same_frame_stay_within_full_scale() {
    let (sink, mut source) = mono_mixer(RATE);
    let click = builtin_click();
    for _ in 0..4 {
        sink.play(&click);
    }
    let out: Vec<f32> = source.by_ref().collect();
    let peak = out.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    assert!(peak <= 1.0, "peak {peak}");
    assert!(peak > 0.0);
}

#[test]
fn a_sample_with_another_rate_plays_for_the_same_duration() {
    let (sink, mut source) = mono_mixer(RATE);
    // 100 ms at 24 kHz into a 48 kHz mixer.
    sink.play(&Sample::new(1, 24_000, vec![0.1; 2_400]));
    let frames = source.by_ref().count();
    let expected = 4_800_f64;
    assert!(
        (frames as f64 - expected).abs() <= expected * 0.05,
        "{frames} frames"
    );
}

#[test]
fn ac8_the_requested_buffer_is_at_most_10_ms() {
    for rate in [44_100_u32, 48_000, 96_000, 192_000] {
        let frames = buffer_frames(rate);
        assert!(frames > 0, "no buffer at {rate} Hz");
        let max_frames = u64::from(rate) * 10 / 1000;
        assert!(
            u64::from(frames) <= max_frames,
            "{frames} frames at {rate} Hz is over 10 ms"
        );
    }
}
