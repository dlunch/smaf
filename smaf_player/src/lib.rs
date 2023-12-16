#![no_std]
extern crate alloc;

mod adpcm;

use smaf::{Channel, PcmDataChunk, ScoreTrack, ScoreTrackChunk, Smaf, SmafChunk};

use self::adpcm::decode_adpcm;

pub trait AudioBackend {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]);
}

struct ScoreTrackPlayer<'a> {
    score_track: &'a ScoreTrack<'a>,
    backend: &'a dyn AudioBackend,
}

impl<'a> ScoreTrackPlayer<'a> {
    pub fn new(score_track: &'a ScoreTrack, backend: &'a dyn AudioBackend) -> Self {
        Self { score_track, backend }
    }

    pub fn play(&self) {
        for chunk in self.score_track.chunks.iter() {
            if let ScoreTrackChunk::PcmData(x) = chunk {
                for chunk in x.iter() {
                    match chunk {
                        PcmDataChunk::WaveData(_, x) => {
                            let decoded = decode_adpcm(x.wave_data);
                            let channel = match x.channel {
                                Channel::Mono => 1,
                                Channel::Stereo => 2,
                            };

                            self.backend.play_wave(channel, x.sampling_freq as _, &decoded)
                        }
                    }
                }
            }
        }
    }
}

pub fn play_smaf(smaf: &Smaf, backend: &dyn AudioBackend) {
    let players = smaf.chunks.iter().filter_map(|x| match x {
        SmafChunk::ScoreTrack(_, x) => Some(ScoreTrackPlayer::new(x, backend)),
        _ => None,
    });

    players.for_each(|x| x.play())
}
