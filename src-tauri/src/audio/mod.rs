//! Audio: plays the sound for a key press.
//!
//! This is the only module that talks to `rodio`, and it does so in two
//! files: `device` (the output) and `decode` (audio files into samples).
//! Everything else here is plain code tested with fakes: the built-in click,
//! the sound sets and their random choice, the choice of a sound with its gain and
//! speed, the player that turns a key event into a sound, the preview
//! phrase, the live state the settings change under them, and the
//! supervisor that keeps an output open as devices come and go.

mod click;
mod decode;
mod device;
mod live;
mod output;
mod player;
mod preview;
mod sample;
mod sounds;
mod voice;

pub use click::builtin_click;
pub use decode::{DecodeError, decode};
pub use live::{Live, MAX_PITCH_VARIATION};
pub use output::{
    AudioError, Backend, OutputHandle, OutputStatus, SinkSlot, StatusHandle, Supervisor,
    spawn_output,
};
pub use player::{Player, Sink};
pub use preview::{GAP, HOLD, PHRASE, Previewer, play_phrase};
pub use sample::Sample;
pub use sounds::SoundSet;
pub use voice::{speed_for, voice};
