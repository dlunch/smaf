#![no_std]
extern crate alloc;

mod chunks;

use alloc::vec::Vec;

use nom::{
    bytes::complete::take,
    combinator::{complete, flat_map, map},
    multi::many0,
    number::complete::be_u32,
    sequence::tuple,
    IResult,
};
use nom_derive::{NomBE, Parse};

use self::chunks::{content_info::ContentsInfoChunk, optional_data::OptionalDataChunk};

type SmafResult<T> = anyhow::Result<T>;

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>), // CNTI
    OptionalData(OptionalDataChunk<'a>), // OPDA
    ScoreTrack(u8, &'a [u8]),            // MTRx
    PCMAudioTrack(u8, &'a [u8]),         // ATRx
}

impl<'a> SmafChunk<'a> {
    fn parse_be(data: &'a [u8]) -> IResult<&[u8], SmafChunk<'a>> {
        map(tuple((take(4usize), flat_map(be_u32, take))), |(tag, data): (&[u8], &[u8])| match tag {
            b"CNTI" => return SmafChunk::ContentsInfo(ContentsInfoChunk::parse_be(data).unwrap().1),
            b"OPDA" => return SmafChunk::OptionalData(OptionalDataChunk::parse_be(data).unwrap().1),
            &[b'M', b'T', b'R', x] => return SmafChunk::ScoreTrack(x, data),
            &[b'A', b'T', b'R', x] => return SmafChunk::PCMAudioTrack(x, data),
            _ => panic!("Unknown chunk"),
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
        Ok(Self::parse_be(file).unwrap().1)
    }
}
