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

use self::chunks::{ContentsInfoChunk, OptionalDataChunk};

type SmafResult<T> = anyhow::Result<T>;

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>), // CNTI
    OptionalData(OptionalDataChunk<'a>), // OPDA
    ScoreTrack(u8, &'a [u8]),            // MTRx
    PCMAudioTrack(u8, &'a [u8]),         // ATRx
}

impl<'a> SmafChunk<'a> {
    fn parse_be(data: &'a [u8]) -> IResult<&[u8], SmafChunk<'a>> {
        map_res(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| {
            Ok::<_, nom::Err<_>>(match tag {
                b"CNTI" => SmafChunk::ContentsInfo(ContentsInfoChunk::parse_be(data)?.1),
                b"OPDA" => SmafChunk::OptionalData(OptionalDataChunk::parse_be(data)?.1),
                &[b'M', b'T', b'R', x] => SmafChunk::ScoreTrack(x, data),
                &[b'A', b'T', b'R', x] => SmafChunk::PCMAudioTrack(x, data),
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
    #[nom(Parse = "many0(complete(SmafChunk::parse_be))")]
    pub chunks: Vec<SmafChunk<'a>>,
    pub crc: u16,
}

impl<'a> Smaf<'a> {
    pub fn parse(file: &'a [u8]) -> SmafResult<Self> {
        Ok(Parse::parse(file).map_err(|e| anyhow::anyhow!("{}", e))?.1)
    }
}
