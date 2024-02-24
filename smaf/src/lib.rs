#![no_std]
extern crate alloc;

mod chunks;
mod constants;
mod smaf;

use alloc::string::String;
use core::result;

#[derive(Debug)]
pub enum SmafError {
    ParseError(String),
}

impl From<SmafError> for anyhow::Error {
    fn from(e: SmafError) -> Self {
        anyhow::anyhow!("{:?}", e)
    }
}

pub type Result<T> = result::Result<T, SmafError>;

pub use self::{
    chunks::{
        MobileStandardSequenceData, PCMAudioSequenceData, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PCMDataChunk, ScoreTrack,
        ScoreTrackChunk, ScoreTrackSequenceEvent, WaveData,
    },
    constants::{BaseBit, Channel, FormatType, PcmWaveFormat, StreamWaveFormat},
    smaf::{Smaf, SmafChunk},
};
