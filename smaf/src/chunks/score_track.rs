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
    chunks::{parse_handy_variable_number, parse_timebase, parse_variable_number},
    constants::{BaseBit, Channel, FormatType, StreamWaveFormat},
};

const SHORT_MOD_VALUES: [u8; 15] = [0x00, 0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48, 0x50, 0x60, 0x70, 0x7f];
const SHORT_EXPRESSION_VALUES: [u8; 15] = [0x00, 0x00, 0x1f, 0x27, 0x2f, 0x37, 0x3f, 0x47, 0x4f, 0x57, 0x5f, 0x67, 0x6f, 0x77, 0x7f];

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
    NoteMessage {
        channel: u8,
        note: u8,
        velocity: Option<u8>,
        gate_time: u32,
    },
    ControlChange {
        channel: u8,
        control: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    BankSelect {
        channel: u8,
        value: u8,
    },
    OctaveShift {
        channel: u8,
        value: u8,
    },
    Modulation {
        channel: u8,
        value: u8,
    },
    PitchBend {
        channel: u8,
        value: u16,
    },
    Volume {
        channel: u8,
        value: u8,
    },
    Pan {
        channel: u8,
        value: u8,
    },
    Expression {
        channel: u8,
        value: u8,
    },
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
                        velocity: None,
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
                        velocity: Some(velocity),
                        gate_time,
                    }
                }
                0xA0..=0xAF => {
                    let (remaining, _) = take(2usize)(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::Nop
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
                0xD0..=0xDF => {
                    let (remaining, _) = u8(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::Nop
                }
                0xE0..=0xEF => {
                    // PitchBend (14-bit value: MSB << 7 | LSB)
                    let channel = status_byte & 0b0000_1111;
                    let (remaining, value_lsb) = u8(remaining)?;
                    let (remaining, value_msb) = u8(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::PitchBend {
                        channel,
                        value: ((value_msb as u16 & 0x7F) << 7) | (value_lsb as u16 & 0x7F),
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
                    } else {
                        ScoreTrackSequenceEvent::Nop
                    }
                }
                _ => ScoreTrackSequenceEvent::Nop,
            };

            result.push(Self { duration, event })
        }

        Ok((data, result))
    }

    pub fn parse_handy(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        Self::parse_handy_like(input, false)
    }

    pub fn parse_softbank(input: &[u8]) -> IResult<&[u8], Vec<Self>> {
        Self::parse_handy_like(input, true)
    }

    fn parse_handy_like(input: &[u8], softbank: bool) -> IResult<&[u8], Vec<Self>> {
        let mut data = input;
        let mut result = Vec::new();
        loop {
            if data.is_empty() {
                break;
            }
            if data.len() >= 4 && data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 0 {
                // end of stream
                data = &data[4..];
                break;
            }

            let (remaining, duration) = parse_handy_variable_number(data)?;
            let (remaining, status_byte) = u8(remaining)?;

            let event = match status_byte {
                0x01..=0xFE => {
                    // note
                    // Voice: 0x1=C#, 0x2=D, ..., 0x9=A, 0xA=A#, 0xB=B, 0xC=C
                    let channel = (status_byte & 0b1100_0000) >> 6;
                    let octave = (status_byte & 0b0011_0000) >> 4;
                    let voice = status_byte & 0b0000_1111;
                    let note_number = octave * 12 + voice;

                    let (remaining, gate_time) = parse_handy_variable_number(remaining)?;
                    data = remaining;

                    ScoreTrackSequenceEvent::NoteMessage {
                        channel,
                        note: note_number,
                        velocity: None,
                        gate_time,
                    }
                }
                0x00 => {
                    let (remaining, next_byte) = u8(remaining)?;
                    data = remaining;

                    let channel = (next_byte & 0b1100_0000) >> 6;
                    let event_type = next_byte & 0b0011_1111;

                    if event_type == 0x00 {
                        let (remaining, _) = u8(remaining)?;
                        data = remaining;
                        ScoreTrackSequenceEvent::Nop
                    } else if (0x01..=0x0e).contains(&event_type) {
                        ScoreTrackSequenceEvent::Expression {
                            channel,
                            value: SHORT_EXPRESSION_VALUES[event_type as usize],
                        }
                    } else if (0x11..=0x1e).contains(&event_type) {
                        ScoreTrackSequenceEvent::PitchBend {
                            channel,
                            value: ((event_type as u16 - 0x10) * 16384 / 16).min(0x3fff),
                        }
                    } else if (0x21..=0x2e).contains(&event_type) {
                        ScoreTrackSequenceEvent::Modulation {
                            channel,
                            value: SHORT_MOD_VALUES[(event_type - 0x20) as usize],
                        }
                    } else if event_type == 0x30 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::ProgramChange { channel, program: value }
                    } else if event_type == 0x31 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::BankSelect { channel, value }
                    } else if event_type == 0x32 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::OctaveShift { channel, value }
                    } else if event_type == 0x33 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Modulation { channel, value }
                    } else if event_type == 0x34 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::PitchBend {
                            channel,
                            value: pitch_bend_byte_to_midi(value),
                        }
                    } else if event_type == 0x36 || event_type == 0x3b {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Expression { channel, value }
                    } else if event_type == 0x37 {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Volume { channel, value }
                    } else if event_type == 0x3a {
                        let (remaining, value) = u8(remaining)?;
                        data = remaining;

                        ScoreTrackSequenceEvent::Pan { channel, value }
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
                        if softbank {
                            let (remaining, length) = u8(remaining)?;
                            let (remaining, exclusive_data) = take(length)(remaining)?;
                            data = remaining;

                            ScoreTrackSequenceEvent::Exclusive(exclusive_data.to_vec())
                        } else {
                            let end = remaining.iter().position(|&x| x == 0xf7).unwrap_or(remaining.len());
                            let exclusive_data = remaining[..end].to_vec();
                            data = if end < remaining.len() {
                                &remaining[end + 1..]
                            } else {
                                &remaining[end..]
                            };

                            ScoreTrackSequenceEvent::Exclusive(exclusive_data)
                        }
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

#[allow(clippy::let_and_return)]
fn parse_mobile_compressed(data: &[u8]) -> IResult<&[u8], Vec<SequenceData>> {
    let (remaining, decoded_len) = be_u32(data)?;
    let decoded =
        huffman_decode(decoded_len as usize, remaining).ok_or_else(|| nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Verify)))?;
    let empty: &[u8] = &[];
    let parsed = match all_consuming(SequenceData::parse_mobile)(&decoded) {
        Ok((_, events)) => Ok((empty, events)),
        Err(_) => Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Verify))),
    };
    parsed
}

fn huffman_decode(decoded_len: usize, src: &[u8]) -> Option<Vec<u8>> {
    const N: usize = 256;

    struct BitReader<'a> {
        data: &'a [u8],
        byte_offset: usize,
        bit_offset: u8,
    }

    impl<'a> BitReader<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self {
                data,
                byte_offset: 0,
                bit_offset: 8,
            }
        }

        fn bit_read(&mut self) -> Option<u8> {
            if self.bit_offset == 8 {
                if self.byte_offset >= self.data.len() {
                    return None;
                }
                self.bit_offset = 0;
            }

            let bit = (self.data[self.byte_offset] >> (7 - self.bit_offset)) & 1;
            self.bit_offset += 1;
            if self.bit_offset == 8 {
                self.byte_offset += 1;
            }
            Some(bit)
        }

        fn bit_n_read(&mut self, n: u8) -> Option<usize> {
            let mut bits = 0usize;
            for _ in 0..n {
                bits = (bits << 1) | (self.bit_read()? as usize);
            }
            Some(bits)
        }
    }

    fn read_tree(reader: &mut BitReader<'_>, left: &mut [usize], right: &mut [usize], avail: &mut usize) -> Option<usize> {
        if reader.bit_read()? == 1 {
            let node = *avail;
            *avail += 1;
            if *avail > 2 * N - 1 {
                return None;
            }
            left[node] = read_tree(reader, left, right, avail)?;
            right[node] = read_tree(reader, left, right, avail)?;
            Some(node)
        } else {
            reader.bit_n_read(8)
        }
    }

    let mut left = [0usize; 2 * N - 1];
    let mut right = [0usize; 2 * N - 1];
    let mut avail = N;
    let mut reader = BitReader::new(src);
    let root = read_tree(&mut reader, &mut left, &mut right, &mut avail)?;

    let mut decoded = Vec::with_capacity(decoded_len);
    for _ in 0..decoded_len {
        let mut node = root;
        while node >= N {
            node = if reader.bit_read()? == 1 { right[node] } else { left[node] };
        }
        decoded.push(node as u8);
    }

    Some(decoded)
}

fn pitch_bend_byte_to_midi(value: u8) -> u16 {
    let offset = ((value as i32) - 128) * 64;
    (8192 + offset).clamp(0, 16383) as u16
}

#[allow(clippy::enum_variant_names)]
pub enum ScoreTrackChunk<'a> {
    SetupData(&'a [u8]),
    SequenceData(Vec<SequenceData>),
    PCMData(Vec<PCMDataChunk<'a>>),
    SeekAndPhraseInfo(&'a [u8]),
    Unknown(&'a [u8], &'a [u8]),
}

impl<'a> ScoreTrackChunk<'a> {
    fn parse(format_type: FormatType, data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"Mtsu" => ScoreTrackChunk::SetupData(data),
                b"Mtsq" => {
                    let parser = match format_type {
                        FormatType::MobileStandardNoCompress => SequenceData::parse_mobile,
                        FormatType::MobileStandardCompress => parse_mobile_compressed,
                        FormatType::HandyPhoneStandard => SequenceData::parse_handy,
                    };
                    ScoreTrackChunk::SequenceData(all_consuming(parser)(data)?.1)
                }
                b"SEQU" => ScoreTrackChunk::SequenceData(all_consuming(SequenceData::parse_softbank)(data)?.1),
                b"Mtsp" => ScoreTrackChunk::PCMData(all_consuming(many0(complete(PCMDataChunk::parse)))(data)?.1),
                b"MspI" => ScoreTrackChunk::SeekAndPhraseInfo(data),
                _ => ScoreTrackChunk::Unknown(tag, data),
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
        // Spec: Data#0 upper 4 bits = Ch0, lower 4 bits = Ch1
        //       Data#1 upper 4 bits = Ch2, lower 4 bits = Ch3
        // be_u16 reads as: (Data#0 << 8) | Data#1
        // So bits 15-12 = Ch0, 11-8 = Ch1, 7-4 = Ch2, 3-0 = Ch3
        let mut result = Vec::new();
        for i in 0..4 {
            let shift = (3 - i) * 4; // Ch0=12, Ch1=8, Ch2=4, Ch3=0
            let data = (raw >> shift) & 0b1111;

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
        FormatType::MobileStandardCompress | FormatType::MobileStandardNoCompress => {
            map(take(16usize), |x: &[u8]| x.iter().map(|&x| ChannelStatus::parse_mobile(x)).collect())(data)?
        }
        FormatType::HandyPhoneStandard => map(be_u16, ChannelStatus::parse_handy)(data)?,
    })
}
