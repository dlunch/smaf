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

#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Channel {
    Mono = 0,
    Stereo = 1,
}

impl From<u8> for Channel {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Mono,
            1 => Self::Stereo,
            _ => panic!("Invalid channel value"),
        }
    }
}

#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Format {
    TwosComplementPCM = 0,
    OffsetBinaryPCM = 1,
    YamahaADPCM = 2,
}

impl From<u8> for Format {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::TwosComplementPCM,
            1 => Self::OffsetBinaryPCM,
            2 => Self::YamahaADPCM,
            _ => panic!("Invalid format value"),
        }
    }
}

#[repr(u8)]
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum BaseBit {
    Bit4 = 0,
    Bit8 = 1,
    Bit12 = 2,
    Bit16 = 3,
}

impl From<u8> for BaseBit {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Bit4,
            1 => Self::Bit8,
            2 => Self::Bit12,
            3 => Self::Bit16,
            _ => panic!("Invalid base bit value"),
        }
    }
}

pub struct WaveData<'a> {
    pub channel: Channel,
    pub format: Format,
    pub base_bit: BaseBit,
    pub sampling_freq: u16,
    pub wave_data: &'a [u8],
}

impl<'a> Parse<&'a [u8]> for WaveData<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((u8, be_u16, rest)), |(wave_type, sampling_freq, wave_data)| {
            let channel = Channel::from((wave_type & 0b10000000) >> 7);
            let format = Format::from((wave_type & 0b01110000) >> 4);
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

#[allow(clippy::enum_variant_names)]
pub enum ScoreTrackChunk<'a> {
    SetupData(&'a [u8]),
    SequenceData(&'a [u8]),
    PcmData(Vec<PcmDataChunk<'a>>),
}

impl<'a> Parse<&'a [u8]> for ScoreTrackChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"Mtsu" => ScoreTrackChunk::SetupData(data),
                b"Mtsq" => ScoreTrackChunk::SequenceData(data),
                b"Mtsp" => ScoreTrackChunk::PcmData(all_consuming(many0(complete(PcmDataChunk::parse)))(data)?.1),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

#[repr(u8)]
#[derive(NomBE, Copy, Clone)]
pub enum FormatType {
    HandyPhoneStandard = 0,
    MobileStandardCompress = 1,
    MobileStandardNoCompress = 2,
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
        FormatType::MobileStandardCompress | FormatType::MobileStandardNoCompress => take(16usize)(data),
        _ => panic!("Unsupported format type"),
    }
}
