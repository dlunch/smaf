use alloc::{format, vec::Vec};

use nom::{
    IResult, Parser,
    bytes::complete::{tag, take},
    combinator::{all_consuming, complete, flat_map, map_res},
    multi::many0,
    number::complete::{be_u16, be_u32},
};

use crate::{
    Result, SmafError,
    chunks::{ContentsInfoChunk, OptionalDataChunk, PCMAudioTrack, ScoreTrack, SequenceData},
};

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>),     // CNTI
    OptionalData(OptionalDataChunk),         // OPDA
    ScoreTrack(u8, ScoreTrack<'a>),          // MTRx
    PCMAudioTrack(u8, PCMAudioTrack<'a>),    // ATRx
    SoftbankSequenceData(Vec<SequenceData>), // SEQU
    Unknown(&'a [u8], &'a [u8]),             // unrecognized chunk (tag, data)
}

impl<'a> SmafChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res((take(4usize), flat_map(be_u32, take)), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"CNTI" => Self::ContentsInfo(all_consuming(ContentsInfoChunk::parse).parse(data)?.1),
                b"OPDA" => Self::OptionalData(OptionalDataChunk { raw: data.to_vec() }),
                &[b'M', b'T', b'R', x] => Self::ScoreTrack(x, all_consuming(ScoreTrack::parse).parse(data)?.1),
                &[b'A', b'T', b'R', x] => Self::PCMAudioTrack(x, all_consuming(PCMAudioTrack::parse).parse(data)?.1),
                b"SEQU" => Self::SoftbankSequenceData(all_consuming(SequenceData::parse_softbank).parse(data)?.1),
                _ => Self::Unknown(tag, data),
            })
        })
        .parse(data)
    }
}

pub struct Smaf<'a> {
    pub magic: &'a [u8],
    pub length: u32,
    pub chunks: Vec<SmafChunk<'a>>,
    pub crc: u16,
}

impl<'a> Smaf<'a> {
    fn parse_file(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (data, (magic, length)) = (tag(&b"MMMD"[..]), be_u32).parse(data)?;
        let (remaining, body) = take(length)(data)?;
        let (_, (chunks, crc)) = all_consuming((many0(complete(SmafChunk::parse)), be_u16)).parse(body)?;

        Ok((remaining, Self { magic, length, chunks, crc }))
    }

    pub fn parse(file: &'a [u8]) -> Result<Self> {
        Ok(Self::parse_file(file).map_err(|e| SmafError::ParseError(format!("{e}")))?.1)
    }
}
