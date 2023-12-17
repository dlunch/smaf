use nom::combinator::rest;
use nom_derive::NomBE;

#[derive(NomBE)]
#[nom(Complete)]
#[nom(Exact)]
pub struct ContentsInfoChunk<'a> {
    pub content_class: u8,
    pub content_type: u8,
    pub content_code_type: u8,
    pub copy_status: u8,
    pub copy_counts: u8,
    #[nom(Parse = "rest")]
    pub option: &'a [u8],
}
