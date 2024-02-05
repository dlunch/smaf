#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use core::time::Duration;

mod adpcm;

use futures::future::join_all;
use smaf::{Channel, PcmDataChunk, ScoreTrack, ScoreTrackChunk, Smaf, SmafChunk};

use self::adpcm::decode_adpcm;

#[async_trait::async_trait]
pub trait AudioBackend {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]);
    fn midi_note_on(&self, channel_id: u8, note: u8, velocity: u8);
    fn midi_note_off(&self, channel_id: u8, note: u8);
    fn midi_set_instrument(&self, channel_id: u8, instrument: u8);
    async fn sleep(&self, duration: Duration);
}

struct ScoreTrackPlayer<'a> {
    score_track: &'a ScoreTrack<'a>,
    backend: &'a dyn AudioBackend,
}

impl<'a> ScoreTrackPlayer<'a> {
    pub fn new(score_track: &'a ScoreTrack, backend: &'a dyn AudioBackend) -> Self {
        Self { score_track, backend }
    }

    pub async fn play(&self) {
        for chunk in self.score_track.chunks.iter() {
            if let ScoreTrackChunk::PcmData(x) = chunk {
                for chunk in x.iter() {
                    match chunk {
                        PcmDataChunk::WaveData(_, x) => {
                            assert!(x.base_bit == smaf::BaseBit::Bit4); // current decoder is 4bit only
                            assert!(x.channel == Channel::Mono); // current decoder is mono only

                            let decoded = decode_adpcm(x.wave_data);
                            let channel = match x.channel {
                                Channel::Mono => 1,
                                Channel::Stereo => 2,
                            };

                            self.backend.play_wave(channel, x.sampling_freq as _, &decoded);

                            let duration = Duration::from_secs_f32(decoded.len() as f32 / x.sampling_freq as f32);
                            self.backend.sleep(duration).await;
                        }
                    }
                }
            }
        }
    }
}

pub async fn play_smaf(smaf: &Smaf<'_>, backend: &dyn AudioBackend) {
    let players = smaf.chunks.iter().filter_map(|x| match x {
        SmafChunk::ScoreTrack(_, x) => Some(ScoreTrackPlayer::new(x, backend)),
        _ => None,
    });

    join_all(players.map(|x| async move { x.play().await })).await;
}
