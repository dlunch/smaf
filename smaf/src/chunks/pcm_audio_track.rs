use nom::combinator::rest;
use nom_derive::NomBE;

#[derive(NomBE)]
pub struct PCMAudioTrackChunk<'a> {
    pub format_type: u8,
    pub sequence_type: u8,
    pub wave_type: u16,
    pub timebase_d: u8,
    pub timebase_g: u8,
    #[nom(Parse = "rest")]
    pub chunks: &'a [u8],
}
