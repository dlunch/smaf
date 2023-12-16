#![no_std]
extern crate alloc;

use smaf::{PcmDataChunk, ScoreTrack, ScoreTrackChunk, Smaf, SmafChunk};

pub trait AudioBackend {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[u8]);
}

#[allow(dead_code)]
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
                        PcmDataChunk::WaveData(_, _) => {
                            todo!()
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
