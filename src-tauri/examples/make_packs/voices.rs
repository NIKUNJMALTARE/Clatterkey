//! The recipe for each pack: which strikes make up each group's sound.
//!
//! A switch press is modelled as up to two impacts: the click or tactile
//! bump of the switch, then the keycap bottoming out on the plate. Each
//! impact is a short noise burst ringing a few resonances (the click leaf,
//! the keycap, the plate and case). Variations nudge every frequency, decay,
//! level and time a little, as no two real presses are the same.

use crate::synth::{Mode, Rng, Strike};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pack {
    Blue,
    Brown,
    Red,
    Typewriter,
}

impl Pack {
    pub const ALL: [Pack; 4] = [Pack::Blue, Pack::Brown, Pack::Red, Pack::Typewriter];

    pub fn id(self) -> &'static str {
        match self {
            Pack::Blue => "blue",
            Pack::Brown => "brown",
            Pack::Red => "red",
            Pack::Typewriter => "typewriter",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pack::Blue => "Blue switches",
            Pack::Brown => "Brown switches",
            Pack::Red => "Red switches",
            Pack::Typewriter => "Typewriter",
        }
    }

    /// How many files each group has, for down and up, in `pack.json` order.
    pub fn layout(self) -> Vec<(Kind, Group, usize)> {
        use Group::*;
        use Kind::*;
        if self == Pack::Typewriter {
            return vec![
                (Down, Default, 5),
                (Down, Space, 3),
                (Down, Enter, 1),
                (Down, Backspace, 3),
                (Down, Modifier, 3),
            ];
        }
        vec![
            (Down, Default, 5),
            (Down, Space, 3),
            (Down, Enter, 3),
            (Down, Backspace, 3),
            (Down, Modifier, 3),
            (Up, Default, 3),
            (Up, Space, 2),
        ]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Down,
    Up,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Down => "down",
            Kind::Up => "up",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Default,
    Space,
    Enter,
    Backspace,
    Modifier,
}

impl Group {
    pub fn name(self) -> &'static str {
        match self {
            Group::Default => "default",
            Group::Space => "space",
            Group::Enter => "enter",
            Group::Backspace => "backspace",
            Group::Modifier => "modifier",
        }
    }
}

/// Relative loudness of a group, before the pack is levelled as a whole.
pub fn group_gain(pack: Pack, kind: Kind, group: Group) -> f64 {
    match (kind, group) {
        (Kind::Up, _) if pack == Pack::Blue => 0.6,
        (Kind::Up, _) => 0.4,
        // The typewriter's space clunk and shift are long and low, so they
        // carry more energy per peak than its letters.
        (_, Group::Space) if pack == Pack::Typewriter => 0.8,
        (_, Group::Modifier) if pack == Pack::Typewriter => 0.65,
        (_, Group::Default) => 1.0,
        (_, Group::Space) => 1.0,
        (_, Group::Enter) => 0.95,
        (_, Group::Backspace) => 0.9,
        (_, Group::Modifier) => 0.75,
    }
}

/// The strikes for one file, and its length in seconds.
pub fn recipe(pack: Pack, kind: Kind, group: Group, rng: &mut Rng) -> (Vec<Strike>, f64) {
    match pack {
        Pack::Typewriter => typewriter(group, rng),
        _ => switch(pack, kind, group, rng),
    }
}

fn strike(at: f64, burst: f64, high_pass: f64, low_pass: f64, direct: f64) -> Strike {
    Strike {
        at,
        burst,
        high_pass,
        low_pass,
        direct,
        modes: Vec::new(),
        level: 1.0,
    }
}

impl Strike {
    fn modes(mut self, modes: &[(f64, f64, f64)]) -> Self {
        self.modes = modes.iter().map(|&(f, d, g)| Mode(f, d, g)).collect();
        self
    }

    fn level(mut self, level: f64) -> Self {
        self.level = level;
        self
    }

    /// Scales every frequency by `pitch` and every decay by `length`.
    fn reshape(mut self, pitch: f64, length: f64) -> Self {
        self.high_pass *= pitch;
        self.low_pass *= pitch;
        self.burst *= length;
        for m in &mut self.modes {
            m.0 *= pitch;
            m.1 *= length;
        }
        self
    }

    /// Nudges every value a little, for one variation.
    fn vary(mut self, rng: &mut Rng) -> Self {
        self.at = (self.at + 0.0015 * rng.signed()).max(0.0);
        self.burst = rng.vary(self.burst, 0.15);
        self.level = rng.vary(self.level, 0.12);
        for m in &mut self.modes {
            m.0 = rng.vary(m.0, 0.05);
            m.1 = rng.vary(m.1, 0.12);
            m.2 = rng.vary(m.2, 0.15);
        }
        self
    }
}

/// Pitch and length scaling of each group relative to a letter key: Space is
/// a long, stabilised key and sounds larger and lower; Enter and Backspace
/// sit between; modifiers are close to letters.
fn group_shape(group: Group) -> (f64, f64) {
    match group {
        Group::Default => (1.0, 1.0),
        Group::Space => (0.55, 1.7),
        Group::Enter => (0.78, 1.3),
        Group::Backspace => (0.85, 1.2),
        Group::Modifier => (0.93, 1.0),
    }
}

fn switch(pack: Pack, kind: Kind, group: Group, rng: &mut Rng) -> (Vec<Strike>, f64) {
    let base: Vec<Strike> = match (pack, kind) {
        (Pack::Blue, Kind::Down) => vec![
            // The click leaf snapping: bright and sharp.
            strike(0.0, 0.0005, 3000.0, 0.0, 0.4).modes(&[
                (5200.0, 0.006, 1.0),
                (3900.0, 0.004, 0.6),
                (7100.0, 0.002, 0.3),
            ]),
            // Bottoming out.
            strike(0.011, 0.0012, 0.0, 5000.0, 0.3)
                .modes(&[
                    (1900.0, 0.010, 0.7),
                    (950.0, 0.016, 0.5),
                    (3200.0, 0.006, 0.3),
                ])
                .level(0.8),
        ],
        (Pack::Blue, Kind::Up) => vec![
            strike(0.0, 0.0004, 3000.0, 0.0, 0.3)
                .modes(&[(4600.0, 0.004, 1.0), (3400.0, 0.003, 0.5)]),
            strike(0.006, 0.0008, 0.0, 4000.0, 0.2)
                .modes(&[(1600.0, 0.006, 0.4)])
                .level(0.4),
        ],
        (Pack::Brown, Kind::Down) => vec![
            // The tactile bump: soft and quiet.
            strike(0.0, 0.0008, 300.0, 3000.0, 0.2)
                .modes(&[(2400.0, 0.004, 0.5)])
                .level(0.25),
            // A rounder bottom-out.
            strike(0.009, 0.0015, 0.0, 4000.0, 0.25).modes(&[
                (2100.0, 0.009, 0.8),
                (1150.0, 0.014, 0.6),
                (600.0, 0.018, 0.35),
            ]),
        ],
        (Pack::Brown, Kind::Up) => vec![
            strike(0.0, 0.0008, 0.0, 3000.0, 0.2)
                .modes(&[(1700.0, 0.006, 0.7), (900.0, 0.008, 0.4)]),
        ],
        (Pack::Red, Kind::Down) => vec![
            // No click: a smooth, deep bottom-out.
            strike(0.002, 0.002, 0.0, 2200.0, 0.2).modes(&[
                (1050.0, 0.016, 0.8),
                (620.0, 0.022, 0.6),
                (320.0, 0.028, 0.4),
                (1800.0, 0.008, 0.2),
            ]),
        ],
        (Pack::Red, Kind::Up) => vec![
            strike(0.0, 0.001, 0.0, 2000.0, 0.2).modes(&[(900.0, 0.008, 0.6), (500.0, 0.010, 0.4)]),
        ],
        (Pack::Typewriter, _) => unreachable!("typewriter has its own recipe"),
    };

    let (pitch, length) = group_shape(group);
    let mut strikes: Vec<Strike> = base
        .into_iter()
        .map(|s| s.reshape(pitch, length).vary(rng))
        .collect();
    if group == Group::Space && kind == Kind::Down {
        // The stabiliser wire rattling a moment after the press.
        strikes.push(
            strike(0.016, 0.0015, 0.0, 1500.0, 0.2)
                .modes(&[(180.0, 0.04, 0.5), (420.0, 0.03, 0.4)])
                .level(0.5)
                .vary(rng),
        );
    }
    (strikes, 0.4)
}

fn typewriter(group: Group, rng: &mut Rng) -> (Vec<Strike>, f64) {
    let (strikes, length) = match group {
        Group::Default => (
            vec![
                // The type bar hitting the platen: hard and metallic.
                strike(0.0, 0.0008, 800.0, 0.0, 0.5).modes(&[
                    (2350.0, 0.030, 0.6),
                    (3720.0, 0.022, 0.5),
                    (5130.0, 0.015, 0.35),
                    (6890.0, 0.010, 0.25),
                    (420.0, 0.030, 0.6),
                ]),
                // The platen's thud.
                strike(0.003, 0.0015, 0.0, 1500.0, 0.3)
                    .modes(&[(160.0, 0.03, 0.6), (310.0, 0.025, 0.4)])
                    .level(0.5),
                // The bar falling back.
                strike(0.045, 0.0005, 1500.0, 0.0, 0.3)
                    .modes(&[(2900.0, 0.008, 0.5)])
                    .level(0.25),
            ],
            0.35,
        ),
        Group::Space => (
            vec![
                // Escapement click, then the dull clunk of the space bar.
                strike(0.0, 0.0005, 2000.0, 0.0, 0.3)
                    .modes(&[(3100.0, 0.006, 0.4)])
                    .level(0.25),
                strike(0.004, 0.002, 0.0, 1200.0, 0.3)
                    .modes(&[(220.0, 0.04, 0.7), (560.0, 0.03, 0.5)]),
            ],
            0.3,
        ),
        Group::Backspace => (
            vec![
                strike(0.0, 0.001, 0.0, 3000.0, 0.3).modes(&[
                    (1500.0, 0.012, 0.6),
                    (2600.0, 0.008, 0.4),
                    (500.0, 0.02, 0.5),
                ]),
                strike(0.035, 0.001, 0.0, 3000.0, 0.3)
                    .modes(&[(1400.0, 0.010, 0.6), (700.0, 0.02, 0.5)])
                    .level(0.5),
            ],
            0.3,
        ),
        Group::Modifier => (
            vec![
                // Shift lifting the type basket, and its latch.
                strike(0.0, 0.002, 0.0, 1500.0, 0.3).modes(&[
                    (280.0, 0.05, 0.7),
                    (750.0, 0.03, 0.5),
                    (1900.0, 0.01, 0.2),
                ]),
                strike(0.06, 0.0005, 2000.0, 0.0, 0.3)
                    .modes(&[(3300.0, 0.005, 0.4)])
                    .level(0.3),
            ],
            0.35,
        ),
        Group::Enter => {
            // Carriage return: the line-space ratchet, the carriage sliding,
            // the thud as it stops, and the bell.
            let mut s = Vec::new();
            for i in 0..6 {
                s.push(
                    strike(0.035 * f64::from(i), 0.0004, 1800.0, 0.0, 0.3)
                        .modes(&[(3000.0, 0.004, 0.6), (1800.0, 0.006, 0.3)])
                        .level(0.5),
                );
            }
            s.push(strike(0.05, 0.12, 100.0, 900.0, 1.0).level(0.08));
            s.push(
                strike(0.30, 0.002, 0.0, 1200.0, 0.3)
                    .modes(&[(200.0, 0.04, 0.6), (480.0, 0.03, 0.3)])
                    .level(0.6),
            );
            s.push(
                strike(0.32, 0.0003, 0.0, 0.0, 0.0)
                    .modes(&[
                        (1760.0, 0.45, 1.0),
                        (4180.0, 0.25, 0.5),
                        (6500.0, 0.12, 0.25),
                        (2950.0, 0.18, 0.2),
                    ])
                    .level(0.9),
            );
            (s, 1.3)
        }
    };
    let strikes = strikes.into_iter().map(|s| s.vary(rng)).collect();
    (strikes, length)
}
