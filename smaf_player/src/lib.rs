#![no_std]
extern crate alloc;

use smaf::Smaf;

pub trait AudioBackend {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[u8]);
}

pub fn play_smaf(_smaf: &Smaf) {
    todo!()
}
