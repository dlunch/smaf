use nom::{IResult, Parser, combinator::rest, number::complete::u8};

pub struct ContentsInfoChunk<'a> {
    pub content_class: u8,
    pub content_type: u8,
    pub content_code_type: u8,
    pub copy_status: u8,
    pub copy_counts: u8,
    pub option: &'a [u8],
}

impl<'a> ContentsInfoChunk<'a> {
    pub(crate) fn parse(data: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (remaining, (content_class, content_type, content_code_type, copy_status, copy_counts, option)) =
            (u8, u8, u8, u8, u8, rest).parse(data)?;
        Ok((
            remaining,
            Self {
                content_class,
                content_type,
                content_code_type,
                copy_status,
                copy_counts,
                option,
            },
        ))
    }
}
