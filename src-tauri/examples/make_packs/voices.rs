//! The recipe for each pack: which parts make up each group's sound.
//!
//! A switch press is built from layered parts, each a short noise burst
//! ringing a few resonances: the switch event (a click leaf, a click jacket
//! or a tactile bump; nothing for a linear), the stem bottoming out, the
//! keycap's own ring, the plate, the case, and on long keys the stabiliser.
//! A key going up is the stem returning to the top. Variations nudge every
//! frequency, decay, level and time a little, as no two real presses are the
//! same.
//!
//! A pack with recorded presses (see `sources.rs`) uses them in place of its
//! switch event and bottom-out, with its synthesised keycap, plate, case and
//! stabiliser layered under them.

use crate::synth::{self, Mode, Rng, Strike};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pack {
    Blue,
    Brown,
    Red,
    Typewriter,
    ThockyLinear,
    FastLinear,
    SharpClicky,
    TactileBump,
}

impl Pack {
    pub const ALL: [Pack; 8] = [
        Pack::Blue,
        Pack::Brown,
        Pack::Red,
        Pack::Typewriter,
        Pack::ThockyLinear,
        Pack::FastLinear,
        Pack::SharpClicky,
        Pack::TactileBump,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Pack::Blue => "blue",
            Pack::Brown => "brown",
            Pack::Red => "red",
            Pack::Typewriter => "typewriter",
            Pack::ThockyLinear => "thocky-linear",
            Pack::FastLinear => "fast-linear",
            Pack::SharpClicky => "sharp-clicky",
            Pack::TactileBump => "tactile-bump",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pack::Blue => "Blue switches",
            Pack::Brown => "Brown switches",
            Pack::Red => "Red switches",
            Pack::Typewriter => "Typewriter",
            Pack::ThockyLinear => "Thocky linear",
            Pack::FastLinear => "Fast linear",
            Pack::SharpClicky => "Sharp clicky",
            Pack::TactileBump => "Tactile bump",
        }
    }

    /// How many files each group has, for down and up, in `pack.json` order.
    pub fn layout(self) -> Vec<(Kind, Group, usize)> {
        use Group::*;
        use Kind::*;
        if self == Pack::Typewriter {
            return vec![
                (Down, Default, 8),
                (Down, Space, 3),
                (Down, Enter, 1),
                (Down, Backspace, 3),
                (Down, Modifier, 3),
            ];
        }
        vec![
            (Down, Default, 8),
            (Down, Space, 3),
            (Down, Enter, 3),
            (Down, Backspace, 3),
            (Down, Modifier, 3),
            (Up, Default, 4),
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

    /// Position in the down layout, used to give each group its own run of
    /// recorded presses.
    fn index(self) -> usize {
        match self {
            Group::Default => 0,
            Group::Space => 1,
            Group::Enter => 2,
            Group::Backspace => 3,
            Group::Modifier => 4,
        }
    }
}

/// What a part of a press stands for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Click leaf, click jacket or tactile bump; linears have none.
    SwitchEvent,
    /// The stem reaching the bottom of the housing.
    BottomOut,
    /// The keycap's own short ring.
    Keycap,
    /// The plate the switch sits in.
    Plate,
    /// The case and desk: the low body of the sound.
    Case,
    /// The stabiliser wire of a long key.
    Stabiliser,
    /// The stem returning to the top, for key-up sounds.
    Return,
}

/// One layer of a sound.
pub struct Part {
    pub role: Role,
    pub strike: Strike,
}

fn part(role: Role, strike: Strike) -> Part {
    Part { role, strike }
}

/// Relative loudness of a group, before the pack is levelled as a whole.
pub fn group_gain(pack: Pack, kind: Kind, group: Group) -> f64 {
    match (kind, group) {
        (Kind::Up, _) if matches!(pack, Pack::Blue | Pack::SharpClicky) => 0.6,
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

/// Renders one file: `variation` counts from 1. `presses` are the pack's
/// recorded presses, empty for a synthesised-only pack.
pub fn render(
    pack: Pack,
    kind: Kind,
    group: Group,
    variation: usize,
    presses: &[Vec<f64>],
    rng: &mut Rng,
) -> Vec<f64> {
    if pack == Pack::Typewriter {
        let (strikes, length) = typewriter(group, rng);
        return synth::render(&strikes, length, rng);
    }
    let recorded = if kind == Kind::Down && !presses.is_empty() {
        let at = (group.index() * 8 + variation - 1) % presses.len();
        Some(&presses[at])
    } else {
        None
    };
    let parts = switch(pack, kind, group, rng);
    let strikes: Vec<Strike> = parts
        .into_iter()
        .filter(|p| recorded.is_none() || !matches!(p.role, Role::SwitchEvent | Role::BottomOut))
        .map(|p| p.strike)
        .collect();
    let mut out = synth::render(&strikes, LENGTH, rng);
    if let Some(press) = recorded {
        let p = synth::peak(press);
        if p > 0.0 {
            synth::mix(&mut out, press, 0.0, 1.0 / p);
        }
    }
    out
}

/// Length of a switch sound before its tail is trimmed, s.
const LENGTH: f64 = 0.4;

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

    fn at(mut self, at: f64) -> Self {
        self.at = at;
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

/// A switch pack's press, before the group shape is applied. Times of the
/// keycap, plate and case are relative to the bottom-out.
struct Press {
    event: Option<Strike>,
    /// When the stem reaches the bottom, s after the press starts.
    travel: f64,
    bottom: Strike,
    keycap: Strike,
    plate: Strike,
    case: Strike,
    up: Vec<Strike>,
}

fn press(pack: Pack) -> Press {
    match pack {
        Pack::Blue => Press {
            // The click leaf snapping: bright and sharp.
            event: Some(strike(0.0, 0.0005, 3000.0, 0.0, 0.4).modes(&[
                (5200.0, 0.006, 1.0),
                (3900.0, 0.004, 0.6),
                (7100.0, 0.002, 0.3),
            ])),
            travel: 0.011,
            bottom: strike(0.0, 0.0012, 0.0, 5000.0, 0.3)
                .modes(&[
                    (1900.0, 0.010, 0.7),
                    (950.0, 0.016, 0.5),
                    (3200.0, 0.006, 0.3),
                ])
                .level(0.8),
            keycap: strike(0.0, 0.0004, 1500.0, 0.0, 0.1)
                .modes(&[(2800.0, 0.004, 0.6)])
                .level(0.3),
            plate: strike(0.0005, 0.0008, 0.0, 3000.0, 0.1)
                .modes(&[(1250.0, 0.012, 0.5), (2500.0, 0.008, 0.3)])
                .level(0.3),
            case: strike(0.001, 0.0015, 0.0, 1200.0, 0.1)
                .modes(&[(420.0, 0.020, 0.5), (210.0, 0.025, 0.3)])
                .level(0.25),
            up: vec![
                strike(0.0, 0.0004, 3000.0, 0.0, 0.3)
                    .modes(&[(4600.0, 0.004, 1.0), (3400.0, 0.003, 0.5)]),
                strike(0.006, 0.0008, 0.0, 4000.0, 0.2)
                    .modes(&[(1600.0, 0.006, 0.4)])
                    .level(0.4),
            ],
        },
        Pack::Brown => Press {
            // The tactile bump: soft and quiet.
            event: Some(
                strike(0.0, 0.0008, 300.0, 3000.0, 0.2)
                    .modes(&[(2400.0, 0.004, 0.5)])
                    .level(0.25),
            ),
            travel: 0.009,
            // A rounder bottom-out.
            bottom: strike(0.0, 0.0015, 0.0, 4000.0, 0.25).modes(&[
                (2100.0, 0.009, 0.8),
                (1150.0, 0.014, 0.6),
                (600.0, 0.018, 0.35),
            ]),
            keycap: strike(0.0, 0.0005, 1000.0, 0.0, 0.1)
                .modes(&[(2200.0, 0.004, 0.5)])
                .level(0.25),
            plate: strike(0.0005, 0.001, 0.0, 2500.0, 0.1)
                .modes(&[(1000.0, 0.012, 0.5), (1900.0, 0.008, 0.3)])
                .level(0.3),
            case: strike(0.001, 0.0015, 0.0, 1000.0, 0.1)
                .modes(&[(380.0, 0.020, 0.5), (190.0, 0.025, 0.3)])
                .level(0.3),
            up: vec![
                strike(0.0, 0.0008, 0.0, 3000.0, 0.2)
                    .modes(&[(1700.0, 0.006, 0.7), (900.0, 0.008, 0.4)]),
            ],
        },
        Pack::Red => Press {
            // No click: a smooth, deep bottom-out.
            event: None,
            travel: 0.002,
            bottom: strike(0.0, 0.002, 0.0, 2200.0, 0.2).modes(&[
                (1050.0, 0.016, 0.8),
                (620.0, 0.022, 0.6),
                (320.0, 0.028, 0.4),
                (1800.0, 0.008, 0.2),
            ]),
            keycap: strike(0.0, 0.0006, 0.0, 2500.0, 0.1)
                .modes(&[(1400.0, 0.005, 0.5)])
                .level(0.2),
            plate: strike(0.0005, 0.001, 0.0, 2000.0, 0.1)
                .modes(&[(800.0, 0.012, 0.5)])
                .level(0.25),
            case: strike(0.001, 0.002, 0.0, 900.0, 0.1)
                .modes(&[(300.0, 0.022, 0.5), (160.0, 0.028, 0.3)])
                .level(0.3),
            up: vec![
                strike(0.0, 0.001, 0.0, 2000.0, 0.2)
                    .modes(&[(900.0, 0.008, 0.6), (500.0, 0.010, 0.4)]),
            ],
        },
        Pack::ThockyLinear => Press {
            // Heavy keycaps on a damped case: deep, rounded and muffled.
            event: None,
            travel: 0.002,
            bottom: strike(0.0, 0.0025, 0.0, 1400.0, 0.15).modes(&[
                (520.0, 0.014, 0.8),
                (340.0, 0.018, 0.6),
                (860.0, 0.008, 0.3),
            ]),
            keycap: strike(0.0, 0.0008, 0.0, 1500.0, 0.1)
                .modes(&[(700.0, 0.006, 0.4)])
                .level(0.4),
            plate: strike(0.0005, 0.0012, 0.0, 1200.0, 0.1)
                .modes(&[(450.0, 0.010, 0.4)])
                .level(0.3),
            // A short, low case ring.
            case: strike(0.001, 0.002, 0.0, 700.0, 0.1)
                .modes(&[(180.0, 0.030, 0.6), (240.0, 0.022, 0.4)])
                .level(0.45),
            up: vec![
                strike(0.0, 0.001, 0.0, 1500.0, 0.15)
                    .modes(&[(600.0, 0.008, 0.6), (320.0, 0.010, 0.4)]),
            ],
        },
        Pack::FastLinear => Press {
            // A light stem with a short travel: a tight, bright clack.
            event: None,
            travel: 0.001,
            bottom: strike(0.0, 0.0006, 800.0, 9000.0, 0.3).modes(&[
                (2600.0, 0.004, 0.8),
                (3800.0, 0.003, 0.5),
                (1700.0, 0.005, 0.4),
            ]),
            keycap: strike(0.0, 0.0003, 1500.0, 0.0, 0.1)
                .modes(&[(3200.0, 0.002, 0.4)])
                .level(0.3),
            plate: strike(0.0003, 0.0005, 0.0, 4000.0, 0.1)
                .modes(&[(1500.0, 0.004, 0.3)])
                .level(0.25),
            // A tightly damped case: little low ring.
            case: strike(0.0005, 0.0008, 0.0, 1500.0, 0.1)
                .modes(&[(500.0, 0.006, 0.2)])
                .level(0.2),
            up: vec![
                strike(0.0, 0.0004, 800.0, 0.0, 0.2)
                    .modes(&[(2800.0, 0.003, 0.6), (1900.0, 0.004, 0.3)]),
            ],
        },
        Pack::SharpClicky => Press {
            // A click jacket: a crisp, high snap a few ms before the bottom.
            event: Some(
                strike(0.0, 0.0003, 4000.0, 0.0, 0.5)
                    .modes(&[
                        (6800.0, 0.004, 1.0),
                        (8400.0, 0.003, 0.6),
                        (5600.0, 0.003, 0.5),
                    ])
                    .level(1.1),
            ),
            travel: 0.0055,
            bottom: strike(0.0, 0.0008, 600.0, 9000.0, 0.3)
                .modes(&[
                    (2600.0, 0.006, 0.7),
                    (4200.0, 0.004, 0.5),
                    (1300.0, 0.008, 0.3),
                ])
                .level(0.75),
            keycap: strike(0.0, 0.0003, 2000.0, 0.0, 0.1)
                .modes(&[(3600.0, 0.003, 0.4)])
                .level(0.3),
            plate: strike(0.0005, 0.0006, 0.0, 4000.0, 0.1)
                .modes(&[(1800.0, 0.008, 0.3)])
                .level(0.25),
            case: strike(0.001, 0.001, 0.0, 1500.0, 0.1)
                .modes(&[(450.0, 0.012, 0.2)])
                .level(0.15),
            // The jacket clicking again on the way up.
            up: vec![
                strike(0.0, 0.0003, 3500.0, 0.0, 0.4)
                    .modes(&[(6000.0, 0.003, 1.0), (4500.0, 0.003, 0.5)]),
                strike(0.004, 0.0006, 0.0, 5000.0, 0.2)
                    .modes(&[(2000.0, 0.005, 0.4)])
                    .level(0.4),
            ],
        },
        Pack::TactileBump => Press {
            // A strong bump, clearly heard, before a full bottom-out.
            event: Some(
                strike(0.0, 0.001, 400.0, 4000.0, 0.25)
                    .modes(&[(1800.0, 0.005, 0.8), (2500.0, 0.004, 0.4)])
                    .level(0.55),
            ),
            travel: 0.0075,
            bottom: strike(0.0, 0.0014, 0.0, 5000.0, 0.25).modes(&[
                (1500.0, 0.010, 0.8),
                (900.0, 0.014, 0.6),
                (2400.0, 0.007, 0.4),
                (500.0, 0.018, 0.3),
            ]),
            keycap: strike(0.0, 0.0005, 1000.0, 0.0, 0.1)
                .modes(&[(1900.0, 0.004, 0.4)])
                .level(0.25),
            plate: strike(0.0005, 0.001, 0.0, 2500.0, 0.1)
                .modes(&[(1100.0, 0.010, 0.3)])
                .level(0.3),
            case: strike(0.001, 0.0015, 0.0, 1000.0, 0.1)
                .modes(&[(330.0, 0.020, 0.4), (170.0, 0.025, 0.2)])
                .level(0.3),
            up: vec![
                strike(0.0, 0.0008, 0.0, 3000.0, 0.2)
                    .modes(&[(1500.0, 0.006, 0.6), (800.0, 0.008, 0.4)]),
            ],
        },
        Pack::Typewriter => unreachable!("typewriter has its own recipe"),
    }
}

fn switch(pack: Pack, kind: Kind, group: Group, rng: &mut Rng) -> Vec<Part> {
    let press = press(pack);
    let mut parts = Vec::new();
    if kind == Kind::Up {
        parts.extend(press.up.into_iter().map(|s| part(Role::Return, s)));
    } else {
        if let Some(event) = press.event {
            parts.push(part(Role::SwitchEvent, event));
        }
        let t = press.travel;
        let shifted = |s: Strike| {
            let at = s.at + t;
            s.at(at)
        };
        parts.push(part(Role::BottomOut, shifted(press.bottom)));
        parts.push(part(Role::Keycap, shifted(press.keycap)));
        parts.push(part(Role::Plate, shifted(press.plate)));
        parts.push(part(Role::Case, shifted(press.case)));
    }

    let (pitch, length) = group_shape(group);
    let mut parts: Vec<Part> = parts
        .into_iter()
        .map(|p| Part {
            role: p.role,
            strike: p.strike.reshape(pitch, length).vary(rng),
        })
        .collect();
    if kind == Kind::Down {
        // The stabiliser wire rattling a moment after a long key's press.
        let level = match group {
            Group::Space => 0.5,
            Group::Enter | Group::Backspace => 0.3,
            Group::Default | Group::Modifier => 0.0,
        };
        if level > 0.0 {
            parts.push(part(
                Role::Stabiliser,
                strike(press.travel + 0.005, 0.0015, 0.0, 1500.0, 0.2)
                    .modes(&[(180.0, 0.04, 0.5), (420.0, 0.03, 0.4)])
                    .level(level)
                    .vary(rng),
            ));
        }
    }
    parts
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
