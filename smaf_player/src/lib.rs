#![no_std]
extern crate alloc;

use alloc::vec::Vec;
mod adpcm;

use smaf::{
    Channel, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PCMDataChunk, ScoreTrack, ScoreTrackChunk, ScoreTrackSequenceEvent, Smaf,
    SmafChunk,
};

use self::adpcm::decode_adpcm;

pub enum SmafEvent {
    Wave { channel: u8, sampling_rate: u32, data: Vec<i16> },
    MidiNoteOn { channel: u8, note: u8, velocity: u8 },
    MidiNoteOff { channel: u8, note: u8, velocity: u8 },
    MidiProgramChange { channel: u8, program: u8 },
    MidiControlChange { channel: u8, control: u8, value: u8 },
    End,
}

pub fn parse_smaf(raw: &[u8]) -> Vec<(usize, SmafEvent)> {
    let smaf = Smaf::parse(raw).unwrap();

    let events = smaf.chunks.iter().filter_map(|x| match x {
        SmafChunk::ScoreTrack(_, x) => Some(parse_score_track_events(x)),
        SmafChunk::PCMAudioTrack(_, x) => Some(parse_pcm_audio_track_events(x)),
        _ => None,
    });

    let mut result = events.flatten().collect::<Vec<_>>();
    result.sort_by_key(|&(time, _)| time);

    result
}

fn parse_score_track_events(track: &ScoreTrack) -> Vec<(usize, SmafEvent)> {
    let sequence_data = track
        .chunks
        .iter()
        .find_map(|chunk| if let ScoreTrackChunk::SequenceData(x) = chunk { Some(x) } else { None })
        .unwrap();

    let mut result = Vec::new();
    let mut now = 0;

    for event in sequence_data.iter() {
        let time = now;
        now += (event.duration * (track.timebase_d as u32)) as usize;

        match event.event {
            ScoreTrackSequenceEvent::NoteMessage {
                channel,
                note,
                velocity,
                gate_time,
            } => {
                // play wave on note 0??
                if note == 0 {
                    let pcm = track
                        .chunks
                        .iter()
                        .find_map(|x| {
                            if let ScoreTrackChunk::PCMData(x) = x {
                                for pcm_chunk in x {
                                    let PCMDataChunk::WaveData(x, y) = pcm_chunk;
                                    if *x == channel + 1 {
                                        return Some(y);
                                    }
                                }
                            }
                            None
                        })
                        .unwrap();

                    assert!(pcm.base_bit == smaf::BaseBit::Bit4); // current decoder is 4bit only
                    assert!(pcm.channel == Channel::Mono); // current decoder is mono only

                    let decoded = decode_adpcm(pcm.wave_data);
                    let channel = match pcm.channel {
                        Channel::Mono => 1,
                        Channel::Stereo => 2,
                    };
                    result.push((
                        time,
                        SmafEvent::Wave {
                            channel,
                            sampling_rate: pcm.sampling_freq as _,
                            data: decoded,
                        },
                    ))
                } else {
                    let duration = (gate_time * (track.timebase_g as u32)) as usize;
                    result.push((time, SmafEvent::MidiNoteOn { channel, note, velocity }));
                    result.push((time + duration, SmafEvent::MidiNoteOff { channel, note, velocity }));
                }
            }
            ScoreTrackSequenceEvent::ControlChange { channel, control, value } => {
                result.push((time, SmafEvent::MidiControlChange { channel, control, value }))
            }
            ScoreTrackSequenceEvent::ProgramChange { channel, program } => result.push((time, SmafEvent::MidiProgramChange { channel, program })),
            ScoreTrackSequenceEvent::Exclusive(_) => continue,
            ScoreTrackSequenceEvent::Nop => continue,
            ScoreTrackSequenceEvent::PitchBend { .. } => continue,
            ScoreTrackSequenceEvent::Volume { .. } => continue,
            ScoreTrackSequenceEvent::Pan { .. } => continue,
            ScoreTrackSequenceEvent::Expression { .. } => continue,
            ScoreTrackSequenceEvent::OctaveShift { .. } => continue,
            ScoreTrackSequenceEvent::Modulation { .. } => continue,
            ScoreTrackSequenceEvent::BankSelect { .. } => continue,
        }
    }
    result.push((now, SmafEvent::End));

    result
}

fn parse_pcm_audio_track_events(track: &PCMAudioTrack) -> Vec<(usize, SmafEvent)> {
    let sequence_data = track
        .chunks
        .iter()
        .find_map(|chunk| {
            if let PCMAudioTrackChunk::SequenceData(x) = chunk {
                Some(x)
            } else {
                None
            }
        })
        .unwrap();

    let mut result = Vec::new();
    let mut now = 0;

    for event in sequence_data.iter() {
        let time = now;
        now += (event.duration * (track.timebase_d as u32)) as usize;

        match event.event {
            PCMAudioSequenceEvent::WaveMessage {
                channel: _,
                wave_number,
                gate_time: _,
            } => {
                let pcm = track
                    .chunks
                    .iter()
                    .find_map(|x| {
                        if let PCMAudioTrackChunk::WaveData(x, y) = x {
                            if *x == wave_number {
                                return Some(y);
                            }
                        }
                        None
                    })
                    .unwrap();

                assert!(track.format == smaf::PcmWaveFormat::Adpcm); // current decoder is adpcm only
                assert!(track.channel == Channel::Mono); // current decoder is mono only

                let decoded = decode_adpcm(pcm);
                let channel = match track.channel {
                    Channel::Mono => 1,
                    Channel::Stereo => 2,
                };
                result.push((
                    time,
                    SmafEvent::Wave {
                        channel,
                        sampling_rate: track.sampling_freq as _,
                        data: decoded,
                    },
                ))
            }
            PCMAudioSequenceEvent::Expression { .. } => continue,
            PCMAudioSequenceEvent::Nop => continue,
            PCMAudioSequenceEvent::Pan { .. } => continue,
            PCMAudioSequenceEvent::PitchBend { .. } => continue,
            PCMAudioSequenceEvent::Volume { .. } => continue,
            PCMAudioSequenceEvent::Exclusive(_) => continue,
        }
    }

    result.push((now, SmafEvent::End));
    result
}
