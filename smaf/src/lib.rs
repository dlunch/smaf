#![no_std]
extern crate alloc;

mod chunks;
mod constants;
mod smaf;

type SmafResult<T> = anyhow::Result<T>;

pub use self::{
    chunks::{PcmDataChunk, ScoreTrack, ScoreTrackChunk},
    constants::{BaseBit, Channel, StreamWaveFormat},
    smaf::{Smaf, SmafChunk},
};
