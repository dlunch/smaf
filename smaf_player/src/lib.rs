#![no_std]
extern crate alloc;

use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    vec,
    vec::Vec,
};
use core::time::Duration;

mod adpcm;

use futures::future::join_all;
use smaf::{Channel, MobileStandardSequenceData, PcmDataChunk, ScoreTrack, ScoreTrackChunk, SequenceEvent, Smaf, SmafChunk, WaveData};

use self::adpcm::decode_adpcm;

#[async_trait::async_trait(?Send)]
pub trait AudioBackend {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]);
    fn midi_note_on(&self, channel_id: u8, note: u8, velocity: u8);
    fn midi_note_off(&self, channel_id: u8, note: u8, velocity: u8);
    fn midi_program_change(&self, channel_id: u8, program: u8);
    fn midi_control_change(&self, channel_id: u8, control: u8, value: u8);
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
        let sequence_data = self.sequence_data();

        let mut now = 0;
        let mut pending_note_off: BTreeMap<u32, Vec<_>> = BTreeMap::new();
        for event in sequence_data {
            let next_event_start = now + event.duration * (self.score_track.timebase_d as u32);

            let next_pending_note_off = pending_note_off.split_off(&(next_event_start));
            for (time, entries) in pending_note_off.into_iter() {
                if time - now > 0 {
                    self.backend.sleep(Duration::from_millis((time - now) as _)).await;
                }

                for (channel, note, velocity) in entries.into_iter() {
                    self.backend.midi_note_off(channel, note, velocity);
                }

                now = time;
            }
            pending_note_off = next_pending_note_off;

            self.backend.sleep(Duration::from_millis((next_event_start - now) as _)).await;
            now = next_event_start;

            match event.event {
                SequenceEvent::NoteMessage {
                    channel,
                    note,
                    velocity,
                    gate_time,
                } => {
                    // play wave on note 0??
                    if note == 0 {
                        let pcm = self.pcm_data(channel + 1);
                        assert!(pcm.base_bit == smaf::BaseBit::Bit4); // current decoder is 4bit only
                        assert!(pcm.channel == Channel::Mono); // current decoder is mono only

                        let decoded = decode_adpcm(pcm.wave_data);
                        let channel = match pcm.channel {
                            Channel::Mono => 1,
                            Channel::Stereo => 2,
                        };
                        self.backend.play_wave(channel, pcm.sampling_freq as _, &decoded);
                    } else {
                        let duration = gate_time * (self.score_track.timebase_g as u32);
                        self.backend.midi_note_on(channel, note, velocity);

                        if let Entry::Vacant(entry) = pending_note_off.entry(now + duration) {
                            entry.insert(vec![(channel, note, velocity)]);
                        } else {
                            pending_note_off.get_mut(&(now + duration)).unwrap().push((channel, note, velocity));
                        }
                    }
                }
                SequenceEvent::ControlChange { channel, control, value } => {
                    self.backend.midi_control_change(channel, control, value);
                }
                SequenceEvent::ProgramChange { channel, program } => {
                    self.backend.midi_program_change(channel, program);
                }
                SequenceEvent::Exclusive(_) => {}
                SequenceEvent::Nop => {}
                SequenceEvent::PitchBend { .. } => {}
            }
        }
    }

    fn sequence_data(&self) -> &[MobileStandardSequenceData] {
        for chunk in self.score_track.chunks.iter() {
            if let ScoreTrackChunk::SequenceData(x) = chunk {
                return x;
            }
        }
        panic!("No sequence data found")
    }

    fn pcm_data(&self, channel: u8) -> &WaveData {
        for chunk in self.score_track.chunks.iter() {
            if let ScoreTrackChunk::PcmData(x) = chunk {
                for pcm_chunk in x {
                    let PcmDataChunk::WaveData(x, y) = pcm_chunk;
                    if *x == channel {
                        return y;
                    }
                }
            }
        }
        panic!("No pcm data found")
    }
}

pub async fn play_smaf(smaf: &Smaf<'_>, backend: &dyn AudioBackend) {
    let players = smaf.chunks.iter().filter_map(|x| match x {
        SmafChunk::ScoreTrack(_, x) => Some(ScoreTrackPlayer::new(x, backend)),
        _ => None,
    });

    join_all(players.map(|x| async move { x.play().await })).await;
}
