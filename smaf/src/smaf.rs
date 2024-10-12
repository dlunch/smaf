use alloc::{format, vec::Vec};

use nom::{
    bytes::complete::take,
    combinator::{all_consuming, complete, flat_map, map_res},
    multi::many0,
    number::complete::be_u32,
    sequence::tuple,
    IResult,
};
use nom_derive::{NomBE, Parse};

use crate::{
    chunks::{ContentsInfoChunk, OptionalDataChunk, PCMAudioTrack, ScoreTrack},
    Result, SmafError,
};

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>),  // CNTI
    OptionalData(OptionalDataChunk<'a>),  // OPDA
    ScoreTrack(u8, ScoreTrack<'a>),       // MTRx
    PCMAudioTrack(u8, PCMAudioTrack<'a>), // ATRx
}

impl<'a> Parse<&'a [u8]> for SmafChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"CNTI" => Self::ContentsInfo(all_consuming(ContentsInfoChunk::parse)(data)?.1),
                b"OPDA" => Self::OptionalData(all_consuming(OptionalDataChunk::parse)(data)?.1),
                &[b'M', b'T', b'R', x] => Self::ScoreTrack(x, all_consuming(ScoreTrack::parse)(data)?.1),
                &[b'A', b'T', b'R', x] => Self::PCMAudioTrack(x, all_consuming(PCMAudioTrack::parse)(data)?.1),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

#[derive(NomBE)]
#[nom(Complete)]
pub struct Smaf<'a> {
    #[nom(Tag(b"MMMD"))]
    pub magic: &'a [u8],
    pub length: u32,
    #[nom(Parse = "many0(complete(SmafChunk::parse))")]
    pub chunks: Vec<SmafChunk<'a>>,
    pub crc: u16,
}

impl<'a> Smaf<'a> {
    pub fn parse(file: &'a [u8]) -> Result<Self> {
        Ok(Parse::parse(file).map_err(|e| SmafError::ParseError(format!("{}", e)))?.1)
    }
}
