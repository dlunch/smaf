#![no_std]
extern crate alloc;

mod chunks;
mod constants;
mod smaf;

type SmafResult<T> = anyhow::Result<T>;

pub use self::{
    chunks::{
        MobileStandardSequenceData, PCMAudioSequenceData, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PCMDataChunk, ScoreTrack,
        ScoreTrackChunk, ScoreTrackSequenceEvent, WaveData,
    },
    constants::{BaseBit, Channel, FormatType, PcmWaveFormat, StreamWaveFormat},
    smaf::{Smaf, SmafChunk},
};
