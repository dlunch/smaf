use crate::SmafResult;

pub struct ContentsInfoChunk<'a> {
    _raw: &'a [u8], // TODO
}

impl<'a> ContentsInfoChunk<'a> {
    pub fn new(raw: &'a [u8]) -> SmafResult<Self> {
        Ok(Self { _raw: raw })
    }
}
