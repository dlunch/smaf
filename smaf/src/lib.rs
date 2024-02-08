#![no_std]
extern crate alloc;

mod chunks;
mod constants;
mod smaf;

type SmafResult<T> = anyhow::Result<T>;

pub use self::{
    chunks::{MobileStandardSequenceData, PCMAudioTrackChunk, PcmDataChunk, ScoreTrack, ScoreTrackChunk, SequenceEvent, WaveData},
    constants::{BaseBit, Channel, FormatType, PcmWaveFormat, StreamWaveFormat},
    smaf::{Smaf, SmafChunk},
};
