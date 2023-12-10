#![no_std]
extern crate alloc;

mod chunks;
mod smaf;

type SmafResult<T> = anyhow::Result<T>;

pub use self::{
    chunks::{PcmDataChunk, ScoreTrack, ScoreTrackChunk},
    smaf::{Smaf, SmafChunk},
};
