mod content_info;
mod optional_data;
mod pcm_audio_track;
mod score_track;

use nom::{number::complete::u8, IResult};

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

pub fn parse_variable_number(input: &[u8]) -> IResult<&[u8], u32> {
    let mut data = input;
    let mut result = 0;
    loop {
        let (remaining, byte) = u8(data)?;
        data = remaining;
        result = (result << 7) | (byte & 0b0111_1111) as u32;
        if byte & 0b1000_0000 == 0 {
            break;
        }
    }

    Ok((data, result))
}

pub use self::{
    content_info::ContentsInfoChunk,
    optional_data::OptionalDataChunk,
    pcm_audio_track::{PCMAudioTrack, PcmAudioTrackChunk},
    score_track::{MobileStandardSequenceData, PcmDataChunk, ScoreTrack, ScoreTrackChunk, SequenceEvent, WaveData},
};
