use alloc::vec::Vec;

use nom::{
    bytes::complete::take,
    combinator::{all_consuming, complete, flat_map, map, map_res, rest},
    multi::many0,
    number::complete::{be_u16, be_u32, u8},
    sequence::tuple,
    IResult,
};
use nom_derive::{NomBE, Parse};

use crate::{
    chunks::{parse_timebase, parse_variable_number},
    constants::{BaseBit, Channel, FormatType, StreamWaveFormat},
};

pub struct WaveData<'a> {
    pub channel: Channel,
    pub format: StreamWaveFormat,
    pub base_bit: BaseBit,
    pub sampling_freq: u16, // in hz
    pub wave_data: &'a [u8],
}

impl<'a> Parse<&'a [u8]> for WaveData<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
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

pub enum PCMDataChunk<'a> {
    WaveData(u8, WaveData<'a>),
}

impl<'a> Parse<&'a [u8]> for PCMDataChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                &[b'M', b'w', b'a', x] => Self::WaveData(x, all_consuming(WaveData::parse)(data)?.1),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

pub enum ScoreTrackSequenceEvent {
    NoteMessage { channel: u8, note: u8, velocity: u8, gate_time: u32 },
    ControlChange { channel: u8, control: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    BankSelect { channel: u8, value: u8 },
    OctaveShift { channel: u8, value: u8 },
    Modulation { channel: u8, value: u8 },
    PitchBend { channel: u8, value: u16 },
    Volume { channel: u8, value: u8 },
    Pan { channel: u8, value: u8 },
    Expression { channel: u8, value: u8 },
    Exclusive(Vec<u8>),
    Nop,
}

pub struct SequenceData {
    pub duration: u32,
    pub event: ScoreTrackSequenceEvent,
}

impl SequenceData {
    pub fn parse_mobile(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        let mut data = input;
        let mut result = Vec::new();
        loop {
            let (remaining, duration) = parse_variable_number(data)?;
            let (remaining, status_byte) = u8(remaining)?;

            let event = match status_byte {
                0x80..=0x8F => {
                    // NoteMessage without velocity
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, note) = u8(remaining)?;
                    let (remaining, gate_time) = parse_variable_number(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::NoteMessage {
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
                    let (remaining, gate_time) = parse_variable_number(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::NoteMessage {
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

                    ScoreTrackSequenceEvent::ControlChange { channel, control, value }
                }
                0xC0..=0xCF => {
                    // ProgramChange
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, program) = u8(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::ProgramChange { channel, program }
                }
                0xE0..=0xEF => {
                    // PitchBend
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, value_lsb) = u8(remaining)?;
                    let (remaining, value_msb) = u8(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::PitchBend {
                        channel,
                        value: (value_msb as u16) << 8 | value_lsb as u16,
                    }
                }
                0xF0 => {
                    // exclusive
                    let (remaining, length) = parse_variable_number(remaining)?;
                    let (remaining, exclusive_data) = take(length)(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::Exclusive(exclusive_data.to_vec())
                }
                0xFF => {
                    // EndOfStream or nop
                    let (remaining, second_byte) = u8(remaining)?;
                    data = remaining;

                    if second_byte == 0x2F {
                        let (remaining, _) = u8(data)?;
                        data = remaining;

                        // XXX dummy nop message to play until end
                        result.push(Self {
                            duration,
                            event: ScoreTrackSequenceEvent::Nop,
                        });

                        break;
                    } else if second_byte == 0x00 {
                        ScoreTrackSequenceEvent::Nop
                    } else {
                        panic!("Invalid status byte");
                    }
                }
                _ => panic!("Invalid status byte {}", status_byte),
            };

            result.push(Self { duration, event })
        }

        Ok((data, result))
    }

    pub fn parse_handy(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        let mut data = input;
        let mut result = Vec::new();
        loop {
            if data.is_empty() {
                break;
            }
            if data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 0 {
                // end of stream
                data = &data[4..];
                break;
            }

            let (remaining, duration) = parse_variable_number(data)?;
            let (remaining, status_byte) = u8(remaining)?;

            let event = match status_byte {
                0x01..=0xFE => {
                    // note
                    let channel = (status_byte & 0b11000000) >> 6;
                    let octave = (status_byte & 0b00110000) >> 4;
                    let note_number = (status_byte & 0b00001111) + octave * 12;

                    let (remaining, gate_time) = parse_variable_number(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::NoteMessage {
                        channel,
                        note: note_number,
                        velocity: 64,
                        gate_time,
                    }
                }
                0x00 => {
                    let (remaining, next_byte) = u8(remaining)?;
                    data = remaining;

                    if next_byte & 0b00111111 == 0b00110000 {
                        // program change

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::ProgramChange {
                            channel: (next_byte & 0b11000000) >> 6,
                            program: value,
                        }
                    } else if next_byte & 0b00111111 == 0b00110001 {
                        // bank select

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::BankSelect {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00111111 == 0b00110010 {
                        // octave shift

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::OctaveShift {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00111111 == 0b00110011 {
                        // modulation

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Modulation {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00111111 == 0b00111000 {
                        // pitch bend

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::PitchBend {
                            channel: (next_byte & 0b11000000) >> 6,
                            value: value as _,
                        }
                    } else if next_byte & 0b00110000 == 0b00010000 {
                        // pitch bend short

                        let value = next_byte & 0b00001111;

                        ScoreTrackSequenceEvent::PitchBend {
                            channel: (next_byte & 0b11000000) >> 6,
                            value: value as _,
                        }
                    } else if next_byte & 0b00111111 == 0b00110111 {
                        // volume

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Volume {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00111111 == 0b00111010 {
                        // pan

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Pan {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00111111 == 0b00111011 {
                        // expression

                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Expression {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else if next_byte & 0b00110000 == 0b0000_0000 {
                        // expression short

                        let value = next_byte & 0b00001111;

                        ScoreTrackSequenceEvent::Expression {
                            channel: (next_byte & 0b11000000) >> 6,
                            value,
                        }
                    } else {
                        // TODO Invalid status byte warning

                        continue;
                    }
                }
                0xFF => {
                    let (remaining, next_byte) = u8(remaining)?;
                    data = remaining;

                    if next_byte == 0b1111_0000 {
                        // exclusive message
                        let (remaining, length) = u8(remaining)?;
                        let (remaining, exclusive_data) = take(length)(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Exclusive(exclusive_data.to_vec())
                    } else if next_byte == 0 {
                        // nop

                        ScoreTrackSequenceEvent::Nop
                    } else {
                        panic!("Invalid status byte");
                    }
                }
            };

            result.push(Self { duration, event })
        }

        Ok((data, result))
    }
}

#[allow(clippy::enum_variant_names)]
pub enum ScoreTrackChunk<'a> {
    SetupData(&'a [u8]),
    SequenceData(Vec<SequenceData>),
    PCMData(Vec<PCMDataChunk<'a>>),
    SeekAndPhraseInfo(&'a [u8]),
}

impl<'a> ScoreTrackChunk<'a> {
    fn parse(format_type: FormatType, data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"Mtsu" => ScoreTrackChunk::SetupData(data),
                b"Mtsq" => {
                    let parser = match format_type {
                        FormatType::MobileStandardNoCompress => SequenceData::parse_mobile,
                        FormatType::HandyPhoneStandard => SequenceData::parse_handy,
                        _ => panic!("Unsupported format type {:?}", format_type),
                    };
                    ScoreTrackChunk::SequenceData(all_consuming(parser)(data)?.1)
                }
                b"Mtsp" => ScoreTrackChunk::PCMData(all_consuming(many0(complete(PCMDataChunk::parse)))(data)?.1),
                b"MspI" => ScoreTrackChunk::SeekAndPhraseInfo(data),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

#[repr(u8)]
pub enum ChannelType {
    NoCare = 0,
    Melody = 1,
    NoMelody = 2,
    Rhythm = 3,
}

impl ChannelType {
    pub fn from_u8(raw: u8) -> Self {
        match raw {
            0 => ChannelType::NoCare,
            1 => ChannelType::Melody,
            2 => ChannelType::NoMelody,
            3 => ChannelType::Rhythm,
            _ => panic!("Invalid channel type"),
        }
    }
}

pub struct ChannelStatus {
    pub kcs: u8, // key control status
    pub vs: u8,  // vibration status
    pub led: u8,
    pub channel_type: ChannelType,
}

impl ChannelStatus {
    pub fn parse_mobile(raw: u8) -> Self {
        let kcs = (raw & 0b1100_0000) >> 6;
        let vs = (raw & 0b0010_0000) >> 5;
        let led = (raw & 0b0001_0000) >> 4;
        let channel_type = raw & 0b0000_0011;

        let channel_type = ChannelType::from_u8(channel_type);

        Self { kcs, vs, led, channel_type }
    }

    pub fn parse_handy(raw: u16) -> Vec<Self> {
        let mut result = Vec::new();
        for i in 0..4 {
            let data = (raw >> (i * 4)) & 0b1111;

            let kcs = (data & 0b1000) >> 3;
            let vs = (data & 0b0100) >> 2;
            let channel_type = data & 0b0011;

            let channel_type = ChannelType::from_u8(channel_type as _);

            result.push(Self {
                kcs: kcs as _,
                vs: vs as _,
                led: 0,
                channel_type,
            });
        }

        result
    }
}

#[derive(NomBE)]
#[nom(Complete)]
#[nom(Exact)]
pub struct ScoreTrack<'a> {
    pub format_type: FormatType,
    pub sequence_type: u8,
    #[nom(Parse = "map(u8, parse_timebase)")]
    pub timebase_d: u8,
    #[nom(Parse = "map(u8, parse_timebase)")]
    pub timebase_g: u8,
    #[nom(Parse = "|x| parse_channel_status(format_type, x)")]
    pub channel_status: Vec<ChannelStatus>,
    #[nom(Parse = "|x| many0(complete(|y| ScoreTrackChunk::parse(format_type, y)))(x)")]
    pub chunks: Vec<ScoreTrackChunk<'a>>,
}

fn parse_channel_status(format_type: FormatType, data: &[u8]) -> IResult<&[u8], Vec<ChannelStatus>> {
    Ok(match format_type {
        FormatType::MobileStandardNoCompress => map(take(16usize), |x: &[u8]| x.iter().map(|&x| ChannelStatus::parse_mobile(x)).collect())(data)?,
        FormatType::HandyPhoneStandard => map(be_u16, ChannelStatus::parse_handy)(data)?,
        _ => panic!("Unsupported format type {:?}", format_type),
    })
}
