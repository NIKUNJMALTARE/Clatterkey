//! Audio: plays the click for a key press.
//!
//! This is the only module that talks to `rodio`, and it does so in one file
//! (`device`). Everything else here is plain code tested with fakes: the
//! built-in click, the player that turns a key event into "play the click",
//! and the supervisor that keeps an output open as devices come and go.

mod click;
mod device;
mod output;
mod player;
mod sample;

pub use click::builtin_click;
pub use output::{
    AudioError, Backend, OutputHandle, OutputStatus, SinkSlot, StatusHandle, Supervisor,
    spawn_output,
};
pub use player::{Player, Sink};
pub use sample::Sample;
