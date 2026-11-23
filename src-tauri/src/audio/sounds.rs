//! The sounds for every key group, for keys going down and up, and the
//! random choice between a group's files.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Sample;
use crate::keys::{KeyGroup, KeyKind};

/// No sample has been chosen from a list yet.
const NONE_YET: usize = usize::MAX;

/// One group's samples, with the last one chosen so it is not chosen twice
/// in a row.
#[derive(Debug)]
struct List {
    samples: Vec<Sample>,
    last: AtomicUsize,
}

impl List {
    fn new(samples: Vec<Sample>) -> Self {
        Self {
            samples,
            last: AtomicUsize::new(NONE_YET),
        }
    }
}

/// Samples for each group, for down and up, with the fallbacks already
/// resolved: an empty down group plays `default`; an empty up group plays
/// up `default`, and is silent if that is empty too.
///
/// A group that falls back plays from the very list it falls back to, so the
/// "not twice in a row" rule holds across them: a letter then Space in a
/// pack with only `default` never plays the same file twice running.
///
/// `pick` runs on the key thread. It takes no lock, allocates nothing and
/// touches no file.
#[derive(Debug)]
pub struct SoundSet {
    /// Each distinct list once.
    lists: Vec<List>,
    /// Which of `lists` each group plays, by `KeyGroup::index`.
    down: [usize; 5],
    /// As `down`; `None` plays nothing.
    up: [Option<usize>; 5],
    rng: AtomicU32,
}

impl SoundSet {
    /// `down` and `up` are indexed by `KeyGroup::index`. `None` if
    /// `down[Default]` is empty: every key must have a sound.
    pub fn new(down: [Vec<Sample>; 5], up: [Vec<Sample>; 5]) -> Option<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
            .unwrap_or(0x9E37_79B9);
        Self::with_seed(down, up, nanos)
    }

    /// As `new`, with a fixed seed for the random choice. For tests.
    #[doc(hidden)]
    pub fn with_seed(down: [Vec<Sample>; 5], up: [Vec<Sample>; 5], seed: u32) -> Option<Self> {
        let default = KeyGroup::Default.index();
        if down[default].is_empty() {
            return None;
        }
        let mut lists = Vec::new();
        let mut add = |samples: Vec<Sample>| {
            lists.push(List::new(samples));
            lists.len() - 1
        };

        let mut down_index = [0; 5];
        let mut down_lists: [Vec<Sample>; 5] = down;
        let down_default = add(std::mem::take(&mut down_lists[default]));
        for group in KeyGroup::ALL {
            let i = group.index();
            down_index[i] = if i == default || down_lists[i].is_empty() {
                down_default
            } else {
                add(std::mem::take(&mut down_lists[i]))
            };
        }

        let mut up_index = [None; 5];
        let mut up_lists: [Vec<Sample>; 5] = up;
        let up_default =
            (!up_lists[default].is_empty()).then(|| add(std::mem::take(&mut up_lists[default])));
        for group in KeyGroup::ALL {
            let i = group.index();
            up_index[i] = if i == default || up_lists[i].is_empty() {
                up_default
            } else {
                Some(add(std::mem::take(&mut up_lists[i])))
            };
        }

        Some(Self {
            lists,
            down: down_index,
            up: up_index,
            // xorshift has one fixed point, zero; never start there.
            rng: AtomicU32::new(if seed == 0 { 0x2545_F491 } else { seed }),
        })
    }

    /// One sample in `default` and nothing for keys going up.
    pub fn single(sample: Sample) -> Self {
        let mut down: [Vec<Sample>; 5] = Default::default();
        down[KeyGroup::Default.index()] = vec![sample];
        Self::new(down, Default::default()).expect("default is not empty")
    }

    /// The sample to play for a key of `group` going `kind`, or `None` if
    /// that plays nothing. With several samples, the one chosen last time
    /// for this group is never chosen again straight away.
    pub fn pick(&self, kind: KeyKind, group: KeyGroup) -> Option<&Sample> {
        let index = match kind {
            KeyKind::Down => Some(self.down[group.index()]),
            KeyKind::Up => self.up[group.index()],
        };
        let list = &self.lists[index?];
        let count = list.samples.len();
        let index = match count {
            0 => return None,
            1 => 0,
            _ => {
                let last = list.last.load(Ordering::Relaxed);
                let random = self.random() as usize;
                if last < count {
                    // One of the other `count - 1`, uniformly.
                    (last + 1 + random % (count - 1)) % count
                } else {
                    random % count
                }
            }
        };
        list.last.store(index, Ordering::Relaxed);
        list.samples.get(index)
    }

    /// The next random number, xorshift32. The key thread calls this, and a
    /// preview may at the same time. A load and a store are enough: a lost
    /// update between the two would only repeat a number.
    pub fn random(&self) -> u32 {
        let mut x = self.rng.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng.store(x, Ordering::Relaxed);
        x
    }
}
