//! Tests for `audio::SoundSet` and its random choice (feature 04, AC3 to
//! AC7 at set level, and AC4 in full). The plan puts these in
//! `audio/sounds.rs`; they are written through the public interface.
//!
//! Signatures assumed from the plan:
//!
//! ```ignore
//! SoundSet::new(down: [Vec<Sample>; 5], up: [Vec<Sample>; 5]) -> Option<SoundSet>
//! SoundSet::with_seed(down, up, seed: u32) -> Option<SoundSet>   // #[doc(hidden)]
//! SoundSet::single(sample: Sample) -> SoundSet
//! SoundSet::pick(&self, kind: KeyKind, group: KeyGroup) -> Option<&Sample>
//! ```

use mechanical_key_sounds_lib::audio::{Sample, SoundSet};
use mechanical_key_sounds_lib::keys::{KeyGroup, KeyKind};

/// A sample whose length (`n` samples) tells it apart from the others.
fn s(n: usize) -> Sample {
    Sample::new(1, 1000, vec![0.5; n])
}

fn none() -> [Vec<Sample>; 5] {
    Default::default()
}

fn at(group: KeyGroup, list: Vec<Sample>) -> [Vec<Sample>; 5] {
    let mut a = none();
    a[group.index()] = list;
    a
}

fn merge(mut a: [Vec<Sample>; 5], b: [Vec<Sample>; 5]) -> [Vec<Sample>; 5] {
    for (i, list) in b.into_iter().enumerate() {
        if !list.is_empty() {
            a[i] = list;
        }
    }
    a
}

fn len_of(set: &SoundSet, kind: KeyKind, group: KeyGroup) -> Option<usize> {
    set.pick(kind, group).map(|s| s.samples().len())
}

#[test]
fn a_set_without_default_down_samples_is_refused() {
    assert!(SoundSet::new(none(), none()).is_none());
    assert!(SoundSet::new(at(KeyGroup::Space, vec![s(1)]), none()).is_none());
    assert!(SoundSet::new(at(KeyGroup::Default, vec![s(1)]), none()).is_some());
}

#[test]
fn ac4_three_files_all_used_and_never_twice_in_a_row_for_many_seeds() {
    for seed in [1u32, 2, 3, 42, 12345, 0xDEAD_BEEF, u32::MAX] {
        let set = SoundSet::with_seed(at(KeyGroup::Default, vec![s(1), s(2), s(3)]), none(), seed)
            .unwrap();
        let picks: Vec<usize> = (0..300)
            .map(|_| len_of(&set, KeyKind::Down, KeyGroup::Default).unwrap())
            .collect();
        for n in 1..=3 {
            assert!(picks.contains(&n), "seed {seed}: file {n} never played");
        }
        for pair in picks.windows(2) {
            assert_ne!(pair[0], pair[1], "seed {seed}: same file twice in a row");
        }
    }
}

#[test]
fn ac4_two_files_alternate() {
    let set = SoundSet::with_seed(at(KeyGroup::Default, vec![s(1), s(2)]), none(), 7).unwrap();
    let picks: Vec<usize> = (0..50)
        .map(|_| len_of(&set, KeyKind::Down, KeyGroup::Default).unwrap())
        .collect();
    for pair in picks.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[test]
fn ac4_one_file_plays_every_time() {
    let only = s(5);
    let set = SoundSet::with_seed(at(KeyGroup::Default, vec![only.clone()]), none(), 9).unwrap();
    for _ in 0..300 {
        let got = set.pick(KeyKind::Down, KeyGroup::Default).unwrap();
        assert!(got.same_buffer(&only));
    }
}

#[test]
fn ac4_the_unseeded_constructor_also_never_repeats() {
    let set = SoundSet::new(at(KeyGroup::Default, vec![s(1), s(2), s(3)]), none()).unwrap();
    let picks: Vec<usize> = (0..300)
        .map(|_| len_of(&set, KeyKind::Down, KeyGroup::Default).unwrap())
        .collect();
    for pair in picks.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[test]
fn ac4_each_group_keeps_its_own_last_choice() {
    let down = merge(
        at(KeyGroup::Default, vec![s(1), s(2), s(3)]),
        at(KeyGroup::Space, vec![s(11), s(12), s(13)]),
    );
    let set = SoundSet::with_seed(down, none(), 3).unwrap();
    let mut last_default = 0;
    let mut last_space = 0;
    for i in 0..200 {
        // Interleave the two groups; each must still never repeat itself.
        let (group, last) = if i % 3 == 0 {
            (KeyGroup::Space, &mut last_space)
        } else {
            (KeyGroup::Default, &mut last_default)
        };
        let n = len_of(&set, KeyKind::Down, group).unwrap();
        assert_ne!(n, *last);
        *last = n;
    }
}

#[test]
fn ac3_a_missing_down_group_uses_default_and_shares_its_buffers() {
    let a = s(1);
    let set = SoundSet::new(at(KeyGroup::Default, vec![a.clone()]), none()).unwrap();
    for group in KeyGroup::ALL {
        let got = set.pick(KeyKind::Down, group).unwrap();
        assert!(got.same_buffer(&a), "{}", group.name());
    }
}

#[test]
fn a_down_group_that_is_present_is_used_instead_of_default() {
    let down = merge(
        at(KeyGroup::Default, vec![s(1)]),
        at(KeyGroup::Space, vec![s(7)]),
    );
    let set = SoundSet::new(down, none()).unwrap();
    assert_eq!(len_of(&set, KeyKind::Down, KeyGroup::Space), Some(7));
    assert_eq!(len_of(&set, KeyKind::Down, KeyGroup::Enter), Some(1));
}

#[test]
fn ac5_up_groups_use_their_own_samples_and_fall_back_to_up_default() {
    let up = merge(
        at(KeyGroup::Default, vec![s(21)]),
        at(KeyGroup::Space, vec![s(22)]),
    );
    let set = SoundSet::new(at(KeyGroup::Default, vec![s(1)]), up).unwrap();
    assert_eq!(len_of(&set, KeyKind::Up, KeyGroup::Space), Some(22));
    assert_eq!(len_of(&set, KeyKind::Up, KeyGroup::Default), Some(21));
    assert_eq!(len_of(&set, KeyKind::Up, KeyGroup::Enter), Some(21));
    assert_eq!(len_of(&set, KeyKind::Up, KeyGroup::Modifier), Some(21));
}

#[test]
fn ac6_up_space_only_leaves_other_up_groups_silent() {
    let set = SoundSet::new(
        at(KeyGroup::Default, vec![s(1)]),
        at(KeyGroup::Space, vec![s(22)]),
    )
    .unwrap();
    assert_eq!(len_of(&set, KeyKind::Up, KeyGroup::Space), Some(22));
    for group in [
        KeyGroup::Default,
        KeyGroup::Enter,
        KeyGroup::Backspace,
        KeyGroup::Modifier,
    ] {
        assert!(set.pick(KeyKind::Up, group).is_none(), "{}", group.name());
    }
}

#[test]
fn ac7_no_up_samples_means_every_up_is_silent_and_never_falls_back_to_down() {
    let set = SoundSet::new(at(KeyGroup::Default, vec![s(1), s(2)]), none()).unwrap();
    for group in KeyGroup::ALL {
        assert!(set.pick(KeyKind::Up, group).is_none(), "{}", group.name());
    }
}

#[test]
fn single_is_the_default_group_only_with_no_up_sounds() {
    let click = s(4);
    let set = SoundSet::single(click.clone());
    for group in KeyGroup::ALL {
        assert!(
            set.pick(KeyKind::Down, group).unwrap().same_buffer(&click),
            "{}",
            group.name()
        );
        assert!(set.pick(KeyKind::Up, group).is_none());
    }
}

#[test]
fn groups_that_fall_back_share_one_no_repeat_sequence_for_down() {
    for seed in [1u32, 5, 99, 0xABCD] {
        let set = SoundSet::with_seed(at(KeyGroup::Default, vec![s(1), s(2), s(3)]), none(), seed)
            .unwrap();
        let groups = [KeyGroup::Default, KeyGroup::Space, KeyGroup::Modifier];
        let mut last = 0;
        for i in 0..300 {
            let n = len_of(&set, KeyKind::Down, groups[i % 3]).unwrap();
            assert_ne!(n, last, "seed {seed}, pick {i}");
            last = n;
        }
    }
}

#[test]
fn groups_that_fall_back_share_one_no_repeat_sequence_for_up() {
    let set = SoundSet::with_seed(
        at(KeyGroup::Default, vec![s(1)]),
        at(KeyGroup::Default, vec![s(21), s(22), s(23)]),
        17,
    )
    .unwrap();
    let groups = [KeyGroup::Default, KeyGroup::Enter, KeyGroup::Backspace];
    let mut last = 0;
    for i in 0..300 {
        let n = len_of(&set, KeyKind::Up, groups[i % 3]).unwrap();
        assert_ne!(n, last, "pick {i}");
        last = n;
    }
}
