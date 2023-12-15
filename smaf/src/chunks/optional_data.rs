use alloc::vec::Vec;
use core::marker::PhantomData;

use nom_derive::NomBE;
#[derive(NomBE)]
#[nom(Complete)]
#[nom(Exact)]
pub struct OptionalDataChunk<'a> {
    pub raw: Vec<u8>,
    phantom: PhantomData<&'a u8>,
}
