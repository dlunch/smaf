use alloc::vec::Vec;

use nom::{
    bytes::complete::take,
    combinator::{all_consuming, complete, flat_map, map_res, rest},
    multi::many0,
    number::complete::{be_u16, be_u32, u8},
    sequence::tuple,
    IResult,
};
use nom_derive::{NomBE, Parse};

use crate::constants::{BaseBit, Channel, FormatType, StreamWaveFormat};

pub struct WaveData<'a> {
    pub channel: Channel,
    pub format: StreamWaveFormat,
    pub base_bit: BaseBit,
    pub sampling_freq: u16, // in hz
    pub wave_data: &'a [u8],
}

impl<'a> Parse<&'a [u8]> for WaveData<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((u8, be_u16, rest)), |(wave_type, sampling_freq, wave_data)| {
            let channel = Channel::from((wave_type & 0b10000000) >> 7);
            let format = StreamWaveFormat::from((wave_type & 0b01110000) >> 4);
            let base_bit = BaseBit::from(wave_type & 0b00001111);

            Ok::<_, nom::Err<nom::error::Error<&'a [u8]>>>(Self {
                channel,
                format,
                base_bit,
                sampling_freq,
                wave_data,
            })
        })(data)
    }
}

pub enum PcmDataChunk<'a> {
    WaveData(u8, WaveData<'a>),
}

impl<'a> Parse<&'a [u8]> for PcmDataChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                &[b'M', b'w', b'a', x] => Self::WaveData(x, all_consuming(WaveData::parse)(data)?.1),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

pub enum SequenceEvent {
    NoteMessage { channel: u8, note: u8, velocity: u8, gate_time: u32 },
    ControlChange { channel: u8, control: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    PitchBend { channel: u8, value_lsb: u8, value_msb: u8 },
    Exclusive(Vec<u8>),
    Nop,
}

pub struct MobileStandardSequenceData {
    pub duration: u32,
    pub event: SequenceEvent,
}

impl MobileStandardSequenceData {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        let mut data = input;
        let mut result = Vec::new();
        loop {
            let (remaining, duration) = Self::parse_variable_number(data)?;
            let (remaining, status_byte) = u8(remaining)?;

            let event = match status_byte {
                0x80..=0x8F => {
                    // NoteMessage without velocity
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, note) = u8(remaining)?;
                    let (remaining, gate_time) = Self::parse_variable_number(remaining)?;
                    data = remaining;

                    SequenceEvent::NoteMessage {
                        channel,
                        note,
                        velocity: 64,
                        gate_time,
                    }
                }
                0x90..=0x9F => {
                    // NoteMessage with velocity
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, note) = u8(remaining)?;
                    let (remaining, velocity) = u8(remaining)?;
                    let (remaining, gate_time) = Self::parse_variable_number(remaining)?;
                    data = remaining;

                    SequenceEvent::NoteMessage {
                        channel,
                        note,
                        velocity,
                        gate_time,
                    }
                }
                0xB0..=0xBF => {
                    // ControlChange
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, control) = u8(remaining)?;
                    let (remaining, value) = u8(remaining)?;
                    data = remaining;

                    SequenceEvent::ControlChange { channel, control, value }
                }
                0xC0..=0xCF => {
                    // ProgramChange
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, program) = u8(remaining)?;
                    data = remaining;

                    SequenceEvent::ProgramChange { channel, program }
                }
                0xE0..=0xEF => {
                    // PitchBend
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, value_lsb) = u8(remaining)?;
                    let (remaining, value_msb) = u8(remaining)?;
                    data = remaining;

                    SequenceEvent::PitchBend {
                        channel,
                        value_lsb,
                        value_msb,
                    }
                }
                0xF0 => {
                    // exclusive
                    let (remaining, length) = Self::parse_variable_number(remaining)?;
                    let (remaining, exclusive_data) = take(length)(remaining)?;
                    data = remaining;

                    SequenceEvent::Exclusive(exclusive_data.to_vec())
                }
                0xFF => {
                    // EndOfStream or nop
                    let (remaining, second_byte) = u8(remaining)?;
                    data = remaining;

                    if second_byte == 0x2F {
                        let (remaining, _) = u8(data)?;
                        data = remaining;
                        break;
                    } else if second_byte == 0x00 {
                        SequenceEvent::Nop
                    } else {
                        panic!("Invalid status byte");
                    }
                }
                _ => panic!("Invalid status byte"),
            };

            result.push(Self { duration, event })
        }

        Ok((data, result))
    }

    fn parse_variable_number(input: &[u8]) -> IResult<&[u8], u32> {
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
}

#[allow(clippy::enum_variant_names)]
pub enum ScoreTrackChunk<'a> {
    SetupData(&'a [u8]),
    SequenceData(Vec<MobileStandardSequenceData>),
    PcmData(Vec<PcmDataChunk<'a>>),
}

impl<'a> Parse<&'a [u8]> for ScoreTrackChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"Mtsu" => ScoreTrackChunk::SetupData(data),
                b"Mtsq" => ScoreTrackChunk::SequenceData(all_consuming(MobileStandardSequenceData::parse)(data)?.1),
                b"Mtsp" => ScoreTrackChunk::PcmData(all_consuming(many0(complete(PcmDataChunk::parse)))(data)?.1),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

#[derive(NomBE)]
#[nom(Complete)]
#[nom(Exact)]
pub struct ScoreTrack<'a> {
    pub format_type: FormatType,
    pub sequence_type: u8,
    pub timebase_d: u8,
    pub timebase_g: u8,
    #[nom(Parse = "{ |x| parse_channel_status(format_type, x) }")]
    pub channel_status: &'a [u8],
    #[nom(Parse = "many0(complete(ScoreTrackChunk::parse))")]
    pub chunks: Vec<ScoreTrackChunk<'a>>,
}

fn parse_channel_status(format_type: FormatType, data: &[u8]) -> IResult<&[u8], &[u8]> {
    match format_type {
        FormatType::MobileStandardNoCompress => take(16usize)(data),
        _ => panic!("Unsupported format type"),
    }
}
