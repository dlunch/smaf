use alloc::vec::Vec;
use nom::{
    bytes::complete::take,
    combinator::{all_consuming, complete, flat_map, map_res},
    multi::many0,
    number::complete::{be_u16, be_u32, u8},
    sequence::tuple,
    IResult,
};
use nom_derive::Parse;

use crate::{
    chunks::{parse_timebase, parse_variable_number},
    constants::{BaseBit, Channel, PcmWaveFormat},
};

const SHORT_PITCH_BEND_VALUES: [u8; 15] = [0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70];
const SHORT_EXPRESSION_VALUES: [u8; 15] = [0x00, 0x00, 0x1f, 0x27, 0x2f, 0x37, 0x3f, 0x47, 0x4f, 0x57, 0x5f, 0x67, 0x6f, 0x77, 0x7f];

pub enum PCMAudioSequenceEvent {
    WaveMessage { channel: u8, wave_number: u8, gate_time: u32 },
    PitchBend { channel: u8, value: u8 },
    Volume { channel: u8, value: u8 },
    Pan { channel: u8, value: u8 },
    Expression { channel: u8, value: u8 },
    Exclusive(Vec<u8>),
    Nop,
}

pub struct PCMAudioSequenceData {
    pub duration: u32,
    pub event: PCMAudioSequenceEvent,
}

impl PCMAudioSequenceData {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        let mut data = input;
        let mut result = Vec::new();
        loop {
            if data.len() == 4 && data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 0 {
                // XXX dummy nop message to play until end
                result.push(Self {
                    duration: 0,
                    event: PCMAudioSequenceEvent::Nop,
                });

                let (remaining, _) = take(4usize)(data)?;
                data = remaining;
                break;
            }

            let (remaining, duration) = parse_variable_number(data)?;
            let (remaining, first_byte) = u8(remaining)?;
            data = remaining;

            if first_byte != 0 {
                if first_byte == 0xff {
                    let (remaining, second_byte) = u8(data)?;
                    data = remaining;

                    if second_byte == 0b1111_0000 {
                        let (remaining, length) = u8(data)?;
                        let (remaining, exclusive_data) = take(length as usize)(remaining)?;
                        data = remaining;

                        result.push(Self {
                            duration,
                            event: PCMAudioSequenceEvent::Exclusive(exclusive_data.to_vec()),
                        });
                    } else if second_byte == 0 {
                        result.push(Self {
                            duration,
                            event: PCMAudioSequenceEvent::Nop,
                        });
                    } else {
                        panic!("Invalid second byte")
                    }
                } else {
                    let channel = first_byte >> 6;
                    let wave_number = first_byte & 0b0011_1111;
                    let (remaining, gate_time) = parse_variable_number(remaining)?;
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::WaveMessage {
                            channel,
                            wave_number,
                            gate_time,
                        },
                    });
                }
            } else {
                let (remaining, second_byte) = u8(data)?;
                let channel = (second_byte & 0b1100_0000) >> 6;
                let event_type = second_byte & 0b0011_1111;

                if (0x01..=0x0e).contains(&event_type) {
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::Expression {
                            channel,
                            value: SHORT_EXPRESSION_VALUES[event_type as usize],
                        },
                    })
                } else if (0x11..=0x1e).contains(&event_type) {
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::PitchBend {
                            channel,
                            value: SHORT_PITCH_BEND_VALUES[(event_type - 0x10) as usize],
                        },
                    })
                } else if event_type == 0x34 {
                    let (remaining, value) = u8(remaining)?;
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::PitchBend { channel, value },
                    })
                } else if event_type == 0x36 || event_type == 0x3b {
                    let (remaining, value) = u8(remaining)?;
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::Expression { channel, value },
                    })
                } else if event_type == 0x37 {
                    let (remaining, value) = u8(remaining)?;
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::Volume { channel, value },
                    })
                } else if event_type == 0x3a {
                    let (remaining, value) = u8(remaining)?;
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::Pan { channel, value },
                    })
                } else if event_type == 0x00 {
                    data = remaining;

                    result.push(Self {
                        duration,
                        event: PCMAudioSequenceEvent::Nop,
                    })
                } else {
                    panic!("Invalid second byte")
                }
            }
        }

        Ok((data, result))
    }
}

pub enum PCMAudioTrackChunk<'a> {
    SeekAndPhraseInfo(&'a [u8]),
    SetupData(&'a [u8]),
    SequenceData(Vec<PCMAudioSequenceData>),
    WaveData(u8, &'a [u8]),
    Unknown(&'a [u8], &'a [u8]),
}

impl<'a> Parse<&'a [u8]> for PCMAudioTrackChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"AspI" => PCMAudioTrackChunk::SeekAndPhraseInfo(data),
                b"Atsu" => PCMAudioTrackChunk::SetupData(data),
                b"Atsq" => PCMAudioTrackChunk::SequenceData(all_consuming(PCMAudioSequenceData::parse)(data)?.1),
                &[b'A', b'w', b'a', x] => PCMAudioTrackChunk::WaveData(x, data),
                _ => PCMAudioTrackChunk::Unknown(tag, data),
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

    pub chunks: Vec<PCMAudioTrackChunk<'a>>,
}

impl<'a> Parse<&'a [u8]> for PCMAudioTrack<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(
            tuple((u8, u8, be_u16, u8, u8, many0(complete(PCMAudioTrackChunk::parse)))),
            |(format_type, sequence_type, wave_type, timebase_d, timebase_g, chunks)| {
                let channel = Channel::from(((wave_type & 0b1000_0000_0000_0000) >> 15) as u8);
                let format = PcmWaveFormat::from(((wave_type & 0b0111_0000_0000_0000) >> 12) as u8);
                let sampling_freq = (wave_type & 0b0000_1111_0000_0000) >> 8;
                let base_bit = BaseBit::from(((wave_type & 0b0000_0000_1111_0000) >> 4) as u8);

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
