use crate::SmafResult;

pub struct OptionalDataChunk<'a> {
    _raw: &'a [u8], // TODO
}

impl<'a> OptionalDataChunk<'a> {
    pub fn new(raw: &'a [u8]) -> SmafResult<Self> {
        Ok(Self { _raw: raw })
    }
}
