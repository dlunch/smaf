#![no_std]
extern crate alloc;

use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
    vec,
    vec::Vec,
};
use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

mod adpcm;

use futures::future::join_all;
use smaf::{
    Channel, MobileStandardSequenceData, PCMAudioSequenceData, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PCMDataChunk, ScoreTrack,
    ScoreTrackChunk, ScoreTrackSequenceEvent, Smaf, SmafChunk, WaveData,
};

use self::adpcm::decode_adpcm;

#[async_trait::async_trait]
pub trait AudioBackend: Sync + Send {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]);
    fn midi_note_on(&self, channel_id: u8, note: u8, velocity: u8);
    fn midi_note_off(&self, channel_id: u8, note: u8, velocity: u8);
    fn midi_program_change(&self, channel_id: u8, program: u8);
    fn midi_control_change(&self, channel_id: u8, control: u8, value: u8);
    async fn sleep(&self, duration: Duration);
    fn now_millis(&self) -> u64;
}

#[async_trait::async_trait]
trait Player {
    async fn play(self, stopped: Arc<AtomicBool>);
}

struct ScoreTrackPlayer<'a> {
    score_track: &'a ScoreTrack<'a>,
    backend: &'a dyn AudioBackend,
}

impl<'a> ScoreTrackPlayer<'a> {
    pub fn new(score_track: &'a ScoreTrack<'a>, backend: &'a dyn AudioBackend) -> Self {
        Self { score_track, backend }
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
            if let ScoreTrackChunk::PCMData(x) = chunk {
                for pcm_chunk in x {
                    let PCMDataChunk::WaveData(x, y) = pcm_chunk;
                    if *x == channel {
                        return y;
                    }
                }
            }
        }
        panic!("No pcm data found")
    }
}

#[async_trait::async_trait]
impl<'a> Player for ScoreTrackPlayer<'a> {
    async fn play(self, stopped: Arc<AtomicBool>) {
        let sequence_data = self.sequence_data();

        let mut now = self.backend.now_millis();
        let mut pending_note_off: BTreeMap<u64, Vec<_>> = BTreeMap::new();
        for event in sequence_data {
            let next_event_start = now + (event.duration * (self.score_track.timebase_d as u32)) as u64;

            let next_pending_note_off = pending_note_off.split_off(&(next_event_start));
            for (time, entries) in pending_note_off.into_iter() {
                if time > now {
                    self.backend.sleep(Duration::from_millis((time - now) as _)).await;
                    now = self.backend.now_millis();
                }

                for (channel, note, velocity) in entries.into_iter() {
                    self.backend.midi_note_off(channel, note, velocity);
                }
            }
            pending_note_off = next_pending_note_off;

            if next_event_start > now {
                self.backend.sleep(Duration::from_millis((next_event_start - now) as _)).await;
            }
            now = self.backend.now_millis();

            if stopped.load(Ordering::Relaxed) {
                return;
            }

            match event.event {
                ScoreTrackSequenceEvent::NoteMessage {
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

                        let end = now + duration as u64;
                        if let Entry::Vacant(entry) = pending_note_off.entry(end) {
                            entry.insert(vec![(channel, note, velocity)]);
                        } else {
                            pending_note_off.get_mut(&(end)).unwrap().push((channel, note, velocity));
                        }
                    }
                }
                ScoreTrackSequenceEvent::ControlChange { channel, control, value } => {
                    self.backend.midi_control_change(channel, control, value);
                }
                ScoreTrackSequenceEvent::ProgramChange { channel, program } => {
                    self.backend.midi_program_change(channel, program);
                }
                ScoreTrackSequenceEvent::Exclusive(_) => {}
                ScoreTrackSequenceEvent::Nop => {}
                ScoreTrackSequenceEvent::PitchBend { .. } => {}
            }
        }
    }
}

#[allow(dead_code)]
struct PCMAudioTrackPlayer<'a> {
    pcm_audio_track: &'a PCMAudioTrack<'a>,
    backend: &'a dyn AudioBackend,
}

impl<'a> PCMAudioTrackPlayer<'a> {
    pub fn new(pcm_audio_track: &'a PCMAudioTrack<'a>, backend: &'a dyn AudioBackend) -> Self {
        Self { pcm_audio_track, backend }
    }

    fn sequence_data(&self) -> &[PCMAudioSequenceData] {
        for chunk in self.pcm_audio_track.chunks.iter() {
            if let PCMAudioTrackChunk::SequenceData(x) = chunk {
                return x;
            }
        }
        panic!("No sequence data found")
    }

    fn wave_data(&self, channel: u8) -> &[u8] {
        for chunk in self.pcm_audio_track.chunks.iter() {
            if let PCMAudioTrackChunk::WaveData(x, y) = chunk {
                if *x == channel {
                    return y;
                }
            }
        }
        panic!("No pcm data found")
    }
}

#[async_trait::async_trait]
impl Player for PCMAudioTrackPlayer<'_> {
    async fn play(self, stopped: Arc<AtomicBool>) {
        let sequence_data = self.sequence_data();

        for event in sequence_data {
            let duration = event.duration * (self.pcm_audio_track.timebase_d as u32);
            self.backend.sleep(Duration::from_millis(duration as _)).await;

            if stopped.load(Ordering::Relaxed) {
                return;
            }

            match event.event {
                PCMAudioSequenceEvent::WaveMessage {
                    channel: _,
                    wave_number,
                    gate_time: _,
                } => {
                    let pcm = self.wave_data(wave_number);
                    assert!(self.pcm_audio_track.format == smaf::PcmWaveFormat::Adpcm); // current decoder is adpcm only
                    assert!(self.pcm_audio_track.channel == Channel::Mono); // current decoder is mono only

                    let decoded = decode_adpcm(pcm);
                    let channel = match self.pcm_audio_track.channel {
                        Channel::Mono => 1,
                        Channel::Stereo => 2,
                    };
                    self.backend.play_wave(channel, self.pcm_audio_track.sampling_freq as _, &decoded);
                }
                PCMAudioSequenceEvent::Expression { .. } => {}
                PCMAudioSequenceEvent::Nop => {}
                PCMAudioSequenceEvent::Pan { .. } => {}
                PCMAudioSequenceEvent::PitchBend { .. } => {}
                PCMAudioSequenceEvent::Volume { .. } => {}
                PCMAudioSequenceEvent::Exclusive(_) => {}
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct SmafPlayer {
    raw: Vec<u8>,
    stopped: Arc<AtomicBool>,
}

impl SmafPlayer {
    pub fn new(raw: Vec<u8>) -> Self {
        Self {
            raw,
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    pub async fn play(&self, backend: &dyn AudioBackend) {
        let smaf = Smaf::parse(&self.raw).unwrap();

        let players = smaf.chunks.iter().filter_map(|x| match x {
            SmafChunk::ScoreTrack(_, x) => Some(ScoreTrackPlayer::new(x, backend).play(self.stopped.clone())),
            SmafChunk::PCMAudioTrack(_, x) => Some(PCMAudioTrackPlayer::new(x, backend).play(self.stopped.clone())),
            _ => None,
        });

        join_all(players).await;
    }
}
