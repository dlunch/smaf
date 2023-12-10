#![no_std]
extern crate alloc;

mod chunks;

use alloc::vec::Vec;

use nom::{
    bytes::complete::take,
    combinator::{complete, flat_map, map_res},
    multi::many0,
    number::complete::be_u32,
    sequence::tuple,
    IResult,
};
use nom_derive::{NomBE, Parse};

pub use self::chunks::{ContentsInfoChunk, OptionalDataChunk, PcmDataChunk, ScoreTrack, ScoreTrackChunk};

type SmafResult<T> = anyhow::Result<T>;

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>), // CNTI
    OptionalData(OptionalDataChunk<'a>), // OPDA
    ScoreTrack(u8, ScoreTrack<'a>),      // MTRx
    PCMAudioTrack(u8, &'a [u8]),         // ATRx
}

impl<'a> Parse<&'a [u8]> for SmafChunk<'a> {
    fn parse(data: &'a [u8]) -> IResult<&[u8], Self> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"CNTI" => Self::ContentsInfo(ContentsInfoChunk::parse(data)?.1),
                b"OPDA" => Self::OptionalData(OptionalDataChunk::parse(data)?.1),
                &[b'M', b'T', b'R', x] => Self::ScoreTrack(x, ScoreTrack::parse(data)?.1),
                &[b'A', b'T', b'R', x] => Self::PCMAudioTrack(x, data),
                _ => return Err(nom::Err::Error(nom::error_position!(data, nom::error::ErrorKind::Switch))),
            })
        })(data)
    }
}

#[derive(NomBE)]
#[nom(Complete)]
#[nom(Exact)]
pub struct Smaf<'a> {
    #[nom(Tag(b"MMMD"))]
    pub magic: &'a [u8],
    pub length: u32,
    #[nom(Parse = "many0(complete(SmafChunk::parse))")]
    pub chunks: Vec<SmafChunk<'a>>,
    pub crc: u16,
}

impl<'a> Smaf<'a> {
    pub fn parse(file: &'a [u8]) -> SmafResult<Self> {
        Ok(Parse::parse(file).map_err(|e| anyhow::anyhow!("{}", e))?.1)
    }
}
