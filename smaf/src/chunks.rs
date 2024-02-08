mod content_info;
mod optional_data;
mod pcm_audio_track;
mod score_track;

pub fn parse_timebase(raw: u8) -> u8 {
    match raw {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 5,
        0x10 => 10,
        0x11 => 20,
        0x12 => 40,
        0x13 => 50,
        _ => panic!("Invalid timebase"),
    }
}

pub use self::{
    content_info::ContentsInfoChunk,
    optional_data::OptionalDataChunk,
    pcm_audio_track::PCMAudioTrackChunk,
    score_track::{MobileStandardSequenceData, PcmDataChunk, ScoreTrack, ScoreTrackChunk, SequenceEvent, WaveData},
};
