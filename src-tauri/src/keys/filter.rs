//! Key events and the auto-repeat filter. Pure code: no OS calls, so it is
//! tested with made-up input.

use std::collections::HashSet;
use std::fmt;

use super::KeyCode;

/// Whether a key went down or came up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyKind {
    Down,
    Up,
}

impl fmt::Display for KeyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            KeyKind::Down => "down",
            KeyKind::Up => "up",
        })
    }
}

/// One key going down or coming up, with auto-repeat already removed.
/// It holds the physical key and nothing else: never the typed character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub kind: KeyKind,
}

/// What an OS back end reports, auto-repeat included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawKey {
    Press(KeyCode),
    Release(KeyCode),
}

/// Removes the auto-repeat of held keys: a key gives one `Down` however long
/// it is held. Each key is tracked on its own.
#[derive(Debug, Default)]
pub struct RepeatFilter {
    held: HashSet<KeyCode>,
}

impl RepeatFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the event to deliver, or `None` for a repeat of a held key.
    /// A release is always delivered, even with no press recorded before it.
    pub fn accept(&mut self, raw: RawKey) -> Option<KeyEvent> {
        match raw {
            RawKey::Press(code) => self.held.insert(code).then_some(KeyEvent {
                code,
                kind: KeyKind::Down,
            }),
            RawKey::Release(code) => {
                self.held.remove(&code);
                Some(KeyEvent {
                    code,
                    kind: KeyKind::Up,
                })
            }
        }
    }
}
