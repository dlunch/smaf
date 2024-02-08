use alloc::vec::Vec;
use nom::{
    bytes::complete::take,
    combinator::{complete, flat_map, map_res},
    multi::many0,
    number::complete::{be_u16, be_u32, u8},
    sequence::tuple,
    IResult,
};
use nom_derive::Parse;

use crate::constants::{BaseBit, Channel, PcmWaveFormat};

use super::parse_timebase;

pub enum PcmAudioTrackChunk<'a> {
    SeekAndPhraseInfo(&'a [u8]),
    SetupData(&'a [u8]),
    SequenceData(&'a [u8]),
    WaveData(u8, &'a [u8]),
}

impl<'a> Parse<&'a [u8]> for PcmAudioTrackChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"AspI" => PcmAudioTrackChunk::SeekAndPhraseInfo(data),
                b"Atsu" => PcmAudioTrackChunk::SetupData(data),
                b"Atsq" => PcmAudioTrackChunk::SequenceData(data),
                &[b'A', b'w', b'a', x] => PcmAudioTrackChunk::WaveData(x, data),
                _ => {
                    return Err(nom::Err::Error::<nom::error::Error<&'a [u8]>>(nom::error_position!(
                        data,
                        nom::error::ErrorKind::Switch
                    )))
                }
            })
        })(data)
    }
}

pub struct PCMAudioTrack<'a> {
    pub format_type: u8,   // should be 0
    pub sequence_type: u8, // 0: stream sequence, 1: sub-sequence
    pub channel: Channel,
    pub format: PcmWaveFormat,
    pub sampling_freq: u16, // in hz
    pub base_bit: BaseBit,
    pub timebase_d: u8, // in ms
    pub timebase_g: u8, // in ms

    pub chunks: Vec<PcmAudioTrackChunk<'a>>,
}

impl<'a> Parse<&'a [u8]> for PCMAudioTrack<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(
            tuple((u8, u8, be_u16, u8, u8, many0(complete(PcmAudioTrackChunk::parse)))),
            |(format_type, sequence_type, wave_type, timebase_d, timebase_g, chunks)| {
                let channel = Channel::from(((wave_type & 0b1000_0000_0000_0000) >> 15) as u8);
                let format = PcmWaveFormat::from(((wave_type & 0b0111_0000_0000_0000) >> 12) as u8);
                let sampling_freq = (wave_type & 0b0000_1111_0000_0000) >> 8;
                let base_bit = BaseBit::from((wave_type & 0b0000_0000_1111_0000) as u8);

                let sampling_freq = match sampling_freq {
                    0 => 4000,
                    1 => 8000,
                    2 => 11000,
                    3 => 22050,
                    4 => 44100,
                    _ => panic!("Invalid sampling frequency {}", sampling_freq),
                };

                let timebase_d = parse_timebase(timebase_d);
                let timebase_g = parse_timebase(timebase_g);

                Ok::<_, nom::Err<nom::error::Error<&'a [u8]>>>(Self {
                    format_type,
                    sequence_type,
                    channel,
                    format,
                    sampling_freq,
                    base_bit,
                    timebase_d,
                    timebase_g,
                    chunks,
                })
            },
        )(data)
    }
}
