#![no_std]
extern crate alloc;

use alloc::vec::Vec;
mod adpcm;

use smaf::{
    Channel, ChannelStatus, ChannelType, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PCMDataChunk, ScoreTrack, ScoreTrackChunk,
    ScoreTrackSequenceEvent, Smaf, SmafChunk,
};

use self::adpcm::decode_adpcm;

pub enum SmafEvent {
    Wave { channel: u8, sampling_rate: u32, data: Vec<i16> },
    MidiNoteOn { channel: u8, note: u8, velocity: u8 },
    MidiNoteOff { channel: u8, note: u8, velocity: u8 },
    MidiProgramChange { channel: u8, program: u8 },
    MidiControlChange { channel: u8, control: u8, value: u8 },
    MidiPitchBend { channel: u8, value: u16 },
    MidiSysEx(Vec<u8>),
    End,
}

pub fn parse_smaf(raw: &[u8]) -> Vec<(usize, SmafEvent)> {
    let Ok(smaf) = Smaf::parse(raw) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    let mut handy_channel_offset = 0;
    let mut handy_tone_map = ToneMap::new();

    for chunk in &smaf.chunks {
        match chunk {
            SmafChunk::ScoreTrack(_, x) => {
                let (events, next_offset) = parse_score_track_events(x, handy_channel_offset, &mut handy_tone_map);
                result.extend(events);
                handy_channel_offset = next_offset;
            }
            SmafChunk::PCMAudioTrack(_, x) => result.extend(parse_pcm_audio_track_events(x)),
            SmafChunk::SoftbankSequenceData(x) => {
                let mut tone_map = ToneMap::new();
                tone_map.init_track(smaf::FormatType::HandyPhoneStandard, &[], handy_channel_offset);
                let (events, next_offset) = parse_sequence_events(x, 20, 20, handy_channel_offset, true, &[], &mut tone_map);
                result.extend(events);
                handy_channel_offset = next_offset;
            }
            _ => {}
        }
    }

    result.sort_by(|(left_time, left_event), (right_time, right_event)| {
        left_time
            .cmp(right_time)
            .then_with(|| event_sort_key(left_event).cmp(&event_sort_key(right_event)))
    });

    result
}

fn event_sort_key(event: &SmafEvent) -> (u8, [u8; 3]) {
    match event {
        SmafEvent::MidiSysEx(data) => (4, [data.first().copied().unwrap_or(0xf0), 0, 0]),
        SmafEvent::MidiControlChange { channel, control, value } => (5, [0xb0 | *channel, *control, *value]),
        SmafEvent::MidiPitchBend { channel, value } => (5, [0xe0 | *channel, (value & 0x7f) as u8, ((value >> 7) & 0x7f) as u8]),
        SmafEvent::MidiProgramChange { channel, program } => (6, [0xc0 | *channel, *program, 0]),
        SmafEvent::MidiNoteOff { channel, note, velocity } => (20, [0x80 | *channel, *note, *velocity]),
        SmafEvent::MidiNoteOn { channel, note, velocity } => (30, [0x90 | *channel, *note, *velocity]),
        SmafEvent::Wave { channel, .. } => (40, [*channel, 0, 0]),
        SmafEvent::End => (99, [0xff, 0x2f, 0]),
    }
}

fn parse_score_track_events(track: &ScoreTrack, handy_channel_offset: u8, handy_tone_map: &mut ToneMap) -> (Vec<(usize, SmafEvent)>, u8) {
    let mut result = Vec::new();
    let mut mobile_tone_map = ToneMap::new();
    let pcm_chunks = track
        .chunks
        .iter()
        .find_map(|chunk| {
            if let ScoreTrackChunk::PCMData(x) = chunk {
                Some(x.as_slice())
            } else {
                None
            }
        })
        .unwrap_or(&[]);
    let is_handy = track.format_type == smaf::FormatType::HandyPhoneStandard;

    if is_handy {
        handy_tone_map.init_track(track.format_type, &track.channel_status, handy_channel_offset);
    } else {
        mobile_tone_map.init_track(track.format_type, &track.channel_status, 0);
    }

    for setup_data in track
        .chunks
        .iter()
        .filter_map(|chunk| if let ScoreTrackChunk::SetupData(x) = chunk { Some(*x) } else { None })
    {
        result.extend(parse_setup_sysex_events(setup_data));
    }

    let tone_map: &mut ToneMap = if is_handy { handy_tone_map } else { &mut mobile_tone_map };

    for sequence_data in track
        .chunks
        .iter()
        .filter_map(|chunk| if let ScoreTrackChunk::SequenceData(x) = chunk { Some(x) } else { None })
    {
        tone_map.preclassify_sequence(sequence_data, is_handy, handy_channel_offset);
        let (events, _) = parse_sequence_events(
            sequence_data,
            track.timebase_d,
            track.timebase_g,
            handy_channel_offset,
            is_handy,
            pcm_chunks,
            &mut *tone_map,
        );
        result.extend(events);
    }

    let next_offset = if is_handy {
        handy_channel_offset.saturating_add(4)
    } else {
        handy_channel_offset
    };

    (result, next_offset)
}

fn parse_sequence_events(
    sequence_data: &[smaf::SequenceData],
    timebase_d: u8,
    timebase_g: u8,
    channel_offset: u8,
    use_channel_offset: bool,
    pcm_chunks: &[PCMDataChunk<'_>],
    tone_map: &mut ToneMap,
) -> (Vec<(usize, SmafEvent)>, u8) {
    let mut result = Vec::new();
    let mut now = 0;
    let mut octave_shift = [0i8; MAX_SMAF_CHANNELS];

    let map_channel = |channel: u8| {
        if use_channel_offset {
            channel.saturating_add(channel_offset)
        } else {
            channel
        }
    };

    for event in sequence_data.iter() {
        now += (event.duration * (timebase_d as u32)) as usize;
        let time = now;

        match event.event {
            ScoreTrackSequenceEvent::NoteMessage {
                channel,
                note,
                velocity,
                gate_time,
            } => {
                let channel = map_channel(channel);
                // play wave on note 0??
                if note == 0 {
                    let pcm = pcm_chunks.iter().find_map(|pcm_chunk| {
                        let PCMDataChunk::WaveData(x, y) = pcm_chunk;
                        if *x == channel + 1 {
                            Some(y)
                        } else {
                            None
                        }
                    });
                    let Some(pcm) = pcm else {
                        continue;
                    };

                    match pcm.format {
                        smaf::StreamWaveFormat::YamahaADPCM => {
                            if pcm.base_bit != smaf::BaseBit::Bit4 || pcm.channel != Channel::Mono {
                                continue;
                            }

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
                        }
                        smaf::StreamWaveFormat::TwosComplementPCM | smaf::StreamWaveFormat::OffsetBinaryPCM => {}
                    }
                } else {
                    let duration = (gate_time * (timebase_g as u32)) as usize;
                    let channel_index = (channel as usize).min(octave_shift.len() - 1);
                    let shifted_note = tone_map.map_note(channel, note as i16 + (octave_shift[channel_index] as i16 * 12));
                    let velocity = tone_map.note_velocity(channel, velocity);
                    let midi_channel = tone_map.real_channel(channel);
                    let duration = tone_map.note_duration(channel, duration);
                    result.push((
                        time,
                        SmafEvent::MidiNoteOn {
                            channel: midi_channel,
                            note: shifted_note,
                            velocity,
                        },
                    ));
                    result.push((
                        time + duration,
                        SmafEvent::MidiNoteOff {
                            channel: midi_channel,
                            note: shifted_note,
                            velocity: 0,
                        },
                    ));
                    result.extend(tone_map.emit_atmosphere_notes(time, duration, channel, shifted_note, velocity));
                }
            }
            ScoreTrackSequenceEvent::ControlChange { channel, control, value } => {
                let channel = map_channel(channel);
                tone_map.update_control(channel, control, value);
                let channel = tone_map.real_channel(channel);
                result.push((time, SmafEvent::MidiControlChange { channel, control, value }))
            }
            ScoreTrackSequenceEvent::ProgramChange { channel, program } => {
                let source_channel = map_channel(channel);
                let (channel, mapped_program) = tone_map.set_program(source_channel, program);
                result.push((
                    time,
                    SmafEvent::MidiProgramChange {
                        channel,
                        program: mapped_program,
                    },
                ));
                result.extend(tone_map.emit_atmosphere_setup(time, source_channel, program));
            }
            ScoreTrackSequenceEvent::Exclusive(ref data) => {
                result.push((time, SmafEvent::MidiSysEx(make_sysex_message(data))));
            }
            ScoreTrackSequenceEvent::Nop => continue,
            ScoreTrackSequenceEvent::PitchBend { channel, value } => {
                let channel = map_channel(channel);
                let channel = tone_map.real_channel(channel);
                result.push((
                    time,
                    SmafEvent::MidiPitchBend {
                        channel,
                        value: value.min(0x3fff),
                    },
                ));
            }
            ScoreTrackSequenceEvent::Volume { channel, value } => {
                let channel = map_channel(channel);
                tone_map.update_control(channel, 7, value);
                if tone_map.format_type == smaf::FormatType::HandyPhoneStandard {
                    if tone_map.is_rhythm(channel) {
                        result.push((
                            time,
                            SmafEvent::MidiControlChange {
                                channel: MIDI_DRUM_CHANNEL,
                                control: 7,
                                value: 100,
                            },
                        ));
                    } else {
                        let midi_channel = tone_map.real_channel(channel);
                        let value = tone_map.effective_volume(channel);
                        result.push((
                            time,
                            SmafEvent::MidiControlChange {
                                channel: midi_channel,
                                control: 7,
                                value,
                            },
                        ));
                    }
                } else {
                    let channel = tone_map.real_channel(channel);
                    result.push((time, SmafEvent::MidiControlChange { channel, control: 7, value }));
                }
            }
            ScoreTrackSequenceEvent::Pan { channel, value } => {
                let channel = map_channel(channel);
                let channel = tone_map.real_channel(channel);
                result.push((time, SmafEvent::MidiControlChange { channel, control: 10, value }));
            }
            ScoreTrackSequenceEvent::Expression { channel, value } => {
                let channel = map_channel(channel);
                if tone_map.format_type == smaf::FormatType::HandyPhoneStandard {
                    let value = tone_map.set_expression(channel, value);
                    if !tone_map.is_rhythm(channel) {
                        let channel = tone_map.real_channel(channel);
                        result.push((time, SmafEvent::MidiControlChange { channel, control: 7, value }));
                    }
                } else {
                    let channel = tone_map.real_channel(channel);
                    result.push((time, SmafEvent::MidiControlChange { channel, control: 11, value }));
                }
            }
            ScoreTrackSequenceEvent::OctaveShift { channel, value } => {
                let channel = map_channel(channel);
                if let Some(value) = parse_octave_shift(value) {
                    let channel_index = (channel as usize).min(octave_shift.len() - 1);
                    octave_shift[channel_index] = value;
                }
            }
            ScoreTrackSequenceEvent::Modulation { channel, value } => {
                let channel = map_channel(channel);
                let channel = tone_map.real_channel(channel);
                result.push((time, SmafEvent::MidiControlChange { channel, control: 1, value }));
            }
            ScoreTrackSequenceEvent::BankSelect { channel, value } => {
                let channel = map_channel(channel);
                tone_map.update_bank_select(channel, value);
                let midi_channel = tone_map.real_channel(channel);
                result.push((
                    time,
                    SmafEvent::MidiControlChange {
                        channel: midi_channel,
                        control: 0,
                        value: value & 0x7f,
                    },
                ));
            }
        }
    }
    result.push((now, SmafEvent::End));

    let next_offset = if use_channel_offset {
        channel_offset.saturating_add(4)
    } else {
        channel_offset
    };

    (result, next_offset)
}

const MIDI_DRUM_CHANNEL: u8 = 9;
const MAX_SMAF_CHANNELS: usize = 64;
const MELODY_ALLOCATION_ORDER: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15];

struct ToneMap {
    format_type: smaf::FormatType,
    channel_types: [u8; MAX_SMAF_CHANNELS],
    programs: [u8; MAX_SMAF_CHANNELS],
    channel_volumes: [u8; MAX_SMAF_CHANNELS],
    expressions: [u8; MAX_SMAF_CHANNELS],
    velocities: [u8; MAX_SMAF_CHANNELS],
    bank_msb: [u8; MAX_SMAF_CHANNELS],
    bank_lsb: [u8; MAX_SMAF_CHANNELS],
    forced_rhythm: [bool; MAX_SMAF_CHANNELS],
    real_map: [Option<u8>; MAX_SMAF_CHANNELS],
    reserved_channels: [bool; 16],
    atmosphere_source: [bool; MAX_SMAF_CHANNELS],
    atmosphere_layers: [[Option<AtmosphereLayer>; 2]; MAX_SMAF_CHANNELS],
}

#[derive(Copy, Clone)]
struct AtmosphereLayer {
    channel: u8,
    program: u8,
    velocity_percent: u8,
    note_offset: i8,
    pan: u8,
    pitch_bend: u16,
    gate_extension_ms: usize,
}

impl ToneMap {
    fn new() -> Self {
        Self {
            format_type: smaf::FormatType::MobileStandardNoCompress,
            channel_types: [2; MAX_SMAF_CHANNELS],
            programs: [0; MAX_SMAF_CHANNELS],
            channel_volumes: [100; MAX_SMAF_CHANNELS],
            expressions: [127; MAX_SMAF_CHANNELS],
            velocities: [64; MAX_SMAF_CHANNELS],
            bank_msb: [0; MAX_SMAF_CHANNELS],
            bank_lsb: [0; MAX_SMAF_CHANNELS],
            forced_rhythm: [false; MAX_SMAF_CHANNELS],
            real_map: [None; MAX_SMAF_CHANNELS],
            reserved_channels: [false; 16],
            atmosphere_source: [false; MAX_SMAF_CHANNELS],
            atmosphere_layers: [[None; 2]; MAX_SMAF_CHANNELS],
        }
    }

    fn init_track(&mut self, format_type: smaf::FormatType, channel_statuses: &[ChannelStatus], channel_offset: u8) {
        self.format_type = format_type;

        if format_type == smaf::FormatType::HandyPhoneStandard {
            let base = (channel_offset as usize).min(MAX_SMAF_CHANNELS - 4);
            for local in 0..4 {
                let channel = base + local;
                self.channel_types[channel] = channel_statuses
                    .get(local)
                    .map(|status| match &status.channel_type {
                        ChannelType::Rhythm => 3,
                        _ => 1,
                    })
                    .unwrap_or(1);
                self.programs[channel] = 0;
                self.channel_volumes[channel] = 100;
                self.expressions[channel] = 127;
                self.velocities[channel] = 127;
                self.bank_msb[channel] = 0;
                self.bank_lsb[channel] = 0;
                self.forced_rhythm[channel] = false;
                self.atmosphere_source[channel] = false;
                self.atmosphere_layers[channel] = [None; 2];
            }
            return;
        }

        self.channel_types = [2; MAX_SMAF_CHANNELS];
        self.programs = [0; MAX_SMAF_CHANNELS];
        self.channel_volumes = [100; MAX_SMAF_CHANNELS];
        self.expressions = [127; MAX_SMAF_CHANNELS];
        self.velocities = [64; MAX_SMAF_CHANNELS];
        self.bank_msb = [0; MAX_SMAF_CHANNELS];
        self.bank_lsb = [0; MAX_SMAF_CHANNELS];
        self.forced_rhythm = [false; MAX_SMAF_CHANNELS];
        self.real_map = [None; MAX_SMAF_CHANNELS];
        self.reserved_channels = [false; 16];
        self.atmosphere_source = [false; MAX_SMAF_CHANNELS];
        self.atmosphere_layers = [[None; 2]; MAX_SMAF_CHANNELS];

        for (channel, status) in channel_statuses.iter().take(16).enumerate() {
            self.channel_types[channel] = match &status.channel_type {
                ChannelType::NoCare | ChannelType::NoMelody => 2,
                ChannelType::Melody => 1,
                ChannelType::Rhythm => 3,
            };
        }
    }

    fn preclassify_sequence(&mut self, sequence_data: &[smaf::SequenceData], use_channel_offset: bool, channel_offset: u8) {
        let mut bank_msb = self.bank_msb;

        for event in sequence_data {
            match event.event {
                ScoreTrackSequenceEvent::ControlChange { channel, control, value } => {
                    let channel = self.logical_channel(channel, use_channel_offset, channel_offset);
                    if control == 0 {
                        bank_msb[channel as usize] = value & 0x7f;
                    }
                }
                ScoreTrackSequenceEvent::BankSelect { channel, value } => {
                    let channel = self.logical_channel(channel, use_channel_offset, channel_offset);
                    bank_msb[channel as usize] = value & 0x7f;
                    if value & 0x80 != 0 {
                        self.forced_rhythm[channel as usize] = true;
                    }
                }
                ScoreTrackSequenceEvent::ProgramChange { channel, .. } => {
                    let channel = self.logical_channel(channel, use_channel_offset, channel_offset);
                    if bank_msb[channel as usize] == 0x7d {
                        self.forced_rhythm[channel as usize] = true;
                    }
                }
                _ => {}
            }
        }
    }

    fn logical_channel(&self, channel: u8, use_channel_offset: bool, channel_offset: u8) -> u8 {
        if use_channel_offset {
            channel.saturating_add(channel_offset).min((MAX_SMAF_CHANNELS - 1) as u8)
        } else {
            channel & 0x0f
        }
    }

    fn pseudo_channel(&self, channel: u8) -> usize {
        if self.format_type == smaf::FormatType::HandyPhoneStandard {
            (channel as usize).min(MAX_SMAF_CHANNELS - 1)
        } else {
            (channel & 0x0f) as usize
        }
    }

    fn is_rhythm(&self, channel: u8) -> bool {
        let channel = self.pseudo_channel(channel);
        self.channel_types[channel] == 3 || self.forced_rhythm[channel] || self.bank_msb[channel] == 0x7d
    }

    fn real_channel(&mut self, channel: u8) -> u8 {
        let channel = self.pseudo_channel(channel);
        if self.is_rhythm(channel as u8) {
            return MIDI_DRUM_CHANNEL;
        }

        if let Some(real_channel) = self.real_map[channel] {
            return real_channel;
        }

        let mut used = [false; 16];
        used[MIDI_DRUM_CHANNEL as usize] = true;
        for real_channel in self.real_map.iter().flatten() {
            used[*real_channel as usize] = true;
        }
        for (channel, reserved) in self.reserved_channels.iter().enumerate() {
            used[channel] |= *reserved;
        }

        let real_channel = MELODY_ALLOCATION_ORDER
            .iter()
            .copied()
            .find(|candidate| !used[*candidate as usize])
            .unwrap_or(channel as u8);
        self.real_map[channel] = Some(real_channel);
        real_channel
    }

    fn update_control(&mut self, channel: u8, control: u8, value: u8) {
        let channel = self.pseudo_channel(channel);
        match control {
            0 => self.bank_msb[channel] = value & 0x7f,
            32 => self.bank_lsb[channel] = value & 0x7f,
            7 => self.channel_volumes[channel] = value.min(0x7f),
            _ => {}
        }
    }

    fn update_bank_select(&mut self, channel: u8, value: u8) {
        let channel = self.pseudo_channel(channel);
        if value & 0x80 != 0 {
            self.forced_rhythm[channel] = true;
        }
        self.bank_msb[channel] = value & 0x7f;
    }

    fn set_program(&mut self, channel: u8, program: u8) -> (u8, u8) {
        let channel = self.pseudo_channel(channel);
        if self.format_type == smaf::FormatType::HandyPhoneStandard && self.channel_types[channel] == 3 {
            self.programs[channel] = program.min(0x7f);
            return (MIDI_DRUM_CHANNEL, 0);
        }

        if self.bank_msb[channel] == 0x7d {
            self.forced_rhythm[channel] = true;
        }

        let program = if self.is_rhythm(channel as u8) {
            0
        } else {
            self.map_program(channel as u8, program)
        };
        self.programs[channel] = program;

        (self.real_channel(channel as u8), program)
    }

    fn map_program(&self, channel: u8, program: u8) -> u8 {
        let channel = self.pseudo_channel(channel);
        match (self.bank_msb[channel], self.bank_lsb[channel], program & 0x7f) {
            (0x7c, 0x01, 0x22) => 81,
            (0x7c, 0x01, 0x70) => 30,
            (0x7c, 0x01, 0x46) => 84,
            (0x7c, 0x01, 0x21) => 33,
            (0x7c, 0x01, 0x6a) => 87,
            (0x7c, 0x01, 0x62) => 98,
            (0x7d, 0x00, 0x02) => 0,
            _ => program & 0x7f,
        }
    }

    fn map_note(&self, channel: u8, note: i16) -> u8 {
        let channel_index = self.pseudo_channel(channel);

        if self.format_type == smaf::FormatType::HandyPhoneStandard {
            if self.is_rhythm(channel) {
                return self.programs[channel_index].min(0x7f);
            }
            return (note + 36).clamp(0, 127) as u8;
        }

        let note = note.clamp(0, 127) as u8;
        if !self.is_rhythm(channel) {
            return note;
        }

        match note {
            0x12 => 45,
            0x1a => 41,
            0x1f => 47,
            0x4d => 50,
            0x54 => 43,
            0x59 => 48,
            _ => note,
        }
    }

    fn note_velocity(&mut self, channel: u8, velocity: Option<u8>) -> u8 {
        let channel = self.pseudo_channel(channel);
        if self.format_type == smaf::FormatType::HandyPhoneStandard && self.channel_types[channel] == 3 {
            return self.hps_drum_velocity(channel);
        }
        if let Some(velocity) = velocity {
            let velocity = velocity.min(0x7f);
            self.velocities[channel] = velocity;
            velocity
        } else {
            self.velocities[channel]
        }
    }

    fn hps_drum_velocity(&self, channel: usize) -> u8 {
        let value = if self.expressions[channel] < 64 {
            (self.channel_volumes[channel] as u16 * self.expressions[channel] as u16) / 102
        } else {
            (self.channel_volumes[channel] as u16 * self.expressions[channel] as u16) / 100
        };

        value.clamp(1, 127) as u8
    }

    fn set_expression(&mut self, channel: u8, value: u8) -> u8 {
        let channel = self.pseudo_channel(channel);
        self.expressions[channel] = value.min(0x7f);
        ((self.channel_volumes[channel] as u16 * self.expressions[channel] as u16) / 127).min(127) as u8
    }

    fn effective_volume(&self, channel: u8) -> u8 {
        let channel = self.pseudo_channel(channel);
        ((self.channel_volumes[channel] as u16 * self.expressions[channel] as u16) / 127).min(127) as u8
    }

    fn note_duration(&self, channel: u8, duration: usize) -> usize {
        if self.atmosphere_source[self.pseudo_channel(channel)] {
            duration + 120
        } else {
            duration
        }
    }

    fn emit_atmosphere_setup(&mut self, time: usize, channel: u8, program: u8) -> Vec<(usize, SmafEvent)> {
        if !self.is_atmosphere_voice(channel, program) {
            return Vec::new();
        }

        let channel_index = self.pseudo_channel(channel);
        self.atmosphere_source[channel_index] = true;

        if self.atmosphere_layers[channel_index][0].is_none() {
            let specs = [
                AtmosphereLayer {
                    channel: 15,
                    program: 99,
                    velocity_percent: 42,
                    note_offset: 0,
                    pan: 52,
                    pitch_bend: 8192 - 170,
                    gate_extension_ms: 220,
                },
                AtmosphereLayer {
                    channel: 14,
                    program: 94,
                    velocity_percent: 30,
                    note_offset: -12,
                    pan: 76,
                    pitch_bend: 8192 + 130,
                    gate_extension_ms: 360,
                },
            ];

            let mut next_layer = 0;
            let mut used = [false; 16];
            used[MIDI_DRUM_CHANNEL as usize] = true;
            for real_channel in self.real_map.iter().flatten() {
                used[*real_channel as usize] = true;
            }
            for (reserved_channel, reserved) in self.reserved_channels.iter().enumerate() {
                used[reserved_channel] |= *reserved;
            }

            for spec in specs {
                if !used[spec.channel as usize] && next_layer < self.atmosphere_layers[channel_index].len() {
                    self.reserved_channels[spec.channel as usize] = true;
                    self.atmosphere_layers[channel_index][next_layer] = Some(spec);
                    next_layer += 1;
                }
            }
        }

        let mut result = Vec::new();
        let source_real_channel = self.real_channel(channel);
        result.push((
            time,
            SmafEvent::MidiControlChange {
                channel: source_real_channel,
                control: 91,
                value: 92,
            },
        ));
        result.push((
            time,
            SmafEvent::MidiControlChange {
                channel: source_real_channel,
                control: 93,
                value: 76,
            },
        ));
        result.push((
            time,
            SmafEvent::MidiControlChange {
                channel: source_real_channel,
                control: 72,
                value: 18,
            },
        ));

        let source_volume = ((self.channel_volumes[channel_index] as u16 * 70) / 100).clamp(1, 127) as u8;
        for layer in self.atmosphere_layers[channel_index].iter().flatten() {
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 7,
                    value: source_volume,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 10,
                    value: layer.pan,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 11,
                    value: 110,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 91,
                    value: 104,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 93,
                    value: 84,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiControlChange {
                    channel: layer.channel,
                    control: 72,
                    value: 22,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiProgramChange {
                    channel: layer.channel,
                    program: layer.program,
                },
            ));
            result.push((
                time,
                SmafEvent::MidiPitchBend {
                    channel: layer.channel,
                    value: layer.pitch_bend,
                },
            ));
        }

        result
    }

    fn emit_atmosphere_notes(&self, time: usize, duration: usize, channel: u8, note: u8, velocity: u8) -> Vec<(usize, SmafEvent)> {
        let channel = self.pseudo_channel(channel);
        if !self.atmosphere_source[channel] {
            return Vec::new();
        }

        let mut result = Vec::new();
        for layer in self.atmosphere_layers[channel].iter().flatten() {
            let note = (note as i16 + layer.note_offset as i16).clamp(0, 127) as u8;
            let velocity = ((velocity as u16 * layer.velocity_percent as u16) / 100).clamp(1, 127) as u8;
            result.push((
                time,
                SmafEvent::MidiNoteOn {
                    channel: layer.channel,
                    note,
                    velocity,
                },
            ));
            result.push((
                time + duration + layer.gate_extension_ms,
                SmafEvent::MidiNoteOff {
                    channel: layer.channel,
                    note,
                    velocity: 0,
                },
            ));
        }

        result
    }

    fn is_atmosphere_voice(&self, channel: u8, program: u8) -> bool {
        let channel = self.pseudo_channel(channel);
        (self.bank_msb[channel], self.bank_lsb[channel], program & 0x7f) == (0x7c, 0x01, 0x62)
    }
}

fn parse_setup_sysex_events(data: &[u8]) -> Vec<(usize, SmafEvent)> {
    let mut result = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        if data[offset] != 0xf0 {
            break;
        }
        offset += 1;

        let Some((length, next_offset)) = read_midi_vlq(data, offset) else {
            break;
        };
        offset = next_offset;

        if offset + length > data.len() {
            break;
        }

        result.push((0, SmafEvent::MidiSysEx(make_sysex_message(&data[offset..offset + length]))));
        offset += length;
    }

    result
}

fn read_midi_vlq(data: &[u8], mut offset: usize) -> Option<(usize, usize)> {
    let mut value = 0usize;
    loop {
        let byte = *data.get(offset)?;
        offset += 1;
        value = (value << 7) | ((byte & 0x7f) as usize);
        if byte & 0x80 == 0 {
            return Some((value, offset));
        }
    }
}

fn make_sysex_message(data: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(data.len() + 2);
    if data.first().copied() != Some(0xf0) {
        message.push(0xf0);
    }
    message.extend_from_slice(data);
    if message.last().copied() != Some(0xf7) {
        message.push(0xf7);
    }
    message
}

fn parse_octave_shift(value: u8) -> Option<i8> {
    match value {
        0x00..=0x04 => Some(value as i8),
        0x81..=0x84 => Some(-((value - 0x80) as i8)),
        _ => None,
    }
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
        now += (event.duration * (track.timebase_d as u32)) as usize;
        let time = now;

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

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{parse_pcm_audio_track_events, parse_sequence_events, SmafEvent, ToneMap};
    use smaf::{
        BaseBit, Channel, ChannelStatus, ChannelType, PCMAudioSequenceData, PCMAudioSequenceEvent, PCMAudioTrack, PCMAudioTrackChunk, PcmWaveFormat,
        ScoreTrackSequenceEvent, SequenceData,
    };

    fn channel_status(channel_type: ChannelType) -> ChannelStatus {
        ChannelStatus {
            kcs: 0,
            vs: 0,
            led: 0,
            channel_type,
        }
    }

    #[test]
    fn maps_yamaha_ma_program_to_gm_fallback() {
        let mut tone_map = ToneMap::new();

        tone_map.update_control(1, 0, 0x7c);
        tone_map.update_control(1, 32, 0x01);

        assert_eq!(tone_map.set_program(1, 0x22), (0, 81));
    }

    #[test]
    fn maps_yamaha_rhythm_bank_to_midi_drum_channel() {
        let mut tone_map = ToneMap::new();

        tone_map.update_control(9, 0, 0x7d);
        tone_map.update_control(9, 32, 0x00);

        assert_eq!(tone_map.set_program(9, 0x02), (9, 0));
        assert_eq!(tone_map.map_note(9, 0x1a), 41);
    }

    #[test]
    fn compacts_melody_channels_around_midi_drum_channel() {
        let mut tone_map = ToneMap::new();

        assert_eq!(tone_map.real_channel(1), 0);
        assert_eq!(tone_map.real_channel(2), 1);
        assert_eq!(tone_map.real_channel(9), 2);

        tone_map.update_control(10, 0, 0x7d);
        assert_eq!(tone_map.real_channel(10), 9);

        assert_eq!(tone_map.real_channel(11), 3);
    }

    #[test]
    fn emits_atmosphere_layers_for_yamaha_ma_ambience_voice() {
        let mut tone_map = ToneMap::new();

        tone_map.update_control(7, 0, 0x7c);
        tone_map.update_control(7, 32, 0x01);
        tone_map.update_control(7, 7, 80);

        let setup = tone_map.emit_atmosphere_setup(0, 7, 0x62);
        assert!(setup
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiProgramChange { channel: 15, program: 99 })));
        assert!(setup.iter().any(|(_, event)| matches!(
            event,
            SmafEvent::MidiControlChange {
                channel: 0,
                control: 91,
                value: 92
            }
        )));
        assert!(setup.iter().any(|(_, event)| matches!(
            event,
            SmafEvent::MidiControlChange {
                channel: 15,
                control: 7,
                value: 56
            }
        )));
        assert!(setup.iter().any(|(_, event)| matches!(
            event,
            SmafEvent::MidiControlChange {
                channel: 15,
                control: 72,
                value: 22
            }
        )));

        let notes = tone_map.emit_atmosphere_notes(100, 200, 7, 84, 64);
        assert!(notes
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiNoteOn { channel: 15, note: 84, .. })));
    }

    #[test]
    fn reuses_previous_velocity_for_mobile_notes_without_velocity() {
        let mut tone_map = ToneMap::new();
        tone_map.init_track(smaf::FormatType::MobileStandardNoCompress, &[], 0);
        let sequence = [
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::NoteMessage {
                    channel: 0,
                    note: 60,
                    velocity: Some(96),
                    gate_time: 10,
                },
            },
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::NoteMessage {
                    channel: 0,
                    note: 62,
                    velocity: None,
                    gate_time: 10,
                },
            },
        ];

        let (events, _) = parse_sequence_events(&sequence, 1, 1, 0, false, &[], &mut tone_map);
        assert!(events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiNoteOn { note: 62, velocity: 96, .. })));
    }

    #[test]
    fn applies_sequence_duration_before_event() {
        let mut tone_map = ToneMap::new();
        tone_map.init_track(smaf::FormatType::MobileStandardNoCompress, &[], 0);
        let sequence = [SequenceData {
            duration: 5,
            event: ScoreTrackSequenceEvent::NoteMessage {
                channel: 0,
                note: 60,
                velocity: Some(64),
                gate_time: 2,
            },
        }];

        let (events, _) = parse_sequence_events(&sequence, 4, 4, 0, false, &[], &mut tone_map);
        assert!(events
            .iter()
            .any(|(time, event)| *time == 20 && matches!(event, SmafEvent::MidiNoteOn { note: 60, .. })));
        assert!(events
            .iter()
            .any(|(time, event)| *time == 28 && matches!(event, SmafEvent::MidiNoteOff { note: 60, .. })));
    }

    #[test]
    fn applies_pcm_sequence_duration_before_event() {
        let track = PCMAudioTrack {
            format_type: 0,
            sequence_type: 0,
            channel: Channel::Mono,
            format: PcmWaveFormat::Adpcm,
            sampling_freq: 8000,
            base_bit: BaseBit::Bit4,
            timebase_d: 4,
            timebase_g: 4,
            chunks: vec![PCMAudioTrackChunk::SequenceData(vec![PCMAudioSequenceData {
                duration: 5,
                event: PCMAudioSequenceEvent::Nop,
            }])],
        };

        let events = parse_pcm_audio_track_events(&track);
        assert!(events.iter().any(|(time, event)| *time == 20 && matches!(event, SmafEvent::End)));
    }

    #[test]
    fn hps_tracks_keep_independent_midi_channel_allocations() {
        let mut tone_map = ToneMap::new();
        let first_track = [channel_status(ChannelType::Melody)];
        tone_map.init_track(smaf::FormatType::HandyPhoneStandard, &first_track, 0);
        let first_sequence = [SequenceData {
            duration: 0,
            event: ScoreTrackSequenceEvent::ProgramChange { channel: 0, program: 40 },
        }];

        let (events, _) = parse_sequence_events(&first_sequence, 1, 1, 0, true, &[], &mut tone_map);
        assert!(events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiProgramChange { channel: 0, program: 40 })));

        let second_track = [channel_status(ChannelType::Melody)];
        tone_map.init_track(smaf::FormatType::HandyPhoneStandard, &second_track, 4);
        let second_sequence = [SequenceData {
            duration: 0,
            event: ScoreTrackSequenceEvent::ProgramChange { channel: 0, program: 41 },
        }];

        let (events, _) = parse_sequence_events(&second_sequence, 1, 1, 4, true, &[], &mut tone_map);
        assert!(events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiProgramChange { channel: 1, program: 41 })));
    }

    #[test]
    fn hps_rhythm_uses_program_as_drum_key_and_expression_velocity() {
        let mut tone_map = ToneMap::new();
        let statuses = [channel_status(ChannelType::Rhythm)];
        tone_map.init_track(smaf::FormatType::HandyPhoneStandard, &statuses, 0);
        let sequence = [
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::ProgramChange { channel: 0, program: 35 },
            },
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::Expression { channel: 0, value: 92 },
            },
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::NoteMessage {
                    channel: 0,
                    note: 1,
                    velocity: None,
                    gate_time: 10,
                },
            },
        ];

        let (events, _) = parse_sequence_events(&sequence, 1, 1, 0, true, &[], &mut tone_map);
        assert!(events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiProgramChange { channel: 9, program: 0 })));
        assert!(events.iter().any(|(_, event)| {
            matches!(
                event,
                SmafEvent::MidiNoteOn {
                    channel: 9,
                    note: 35,
                    velocity: 92
                }
            )
        }));
        assert!(!events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiControlChange { channel: 9, control: 11, .. })));
    }

    #[test]
    fn hps_melody_expression_is_folded_into_volume() {
        let mut tone_map = ToneMap::new();
        let statuses = [channel_status(ChannelType::Melody)];
        tone_map.init_track(smaf::FormatType::HandyPhoneStandard, &statuses, 0);
        let sequence = [
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::Volume { channel: 0, value: 100 },
            },
            SequenceData {
                duration: 0,
                event: ScoreTrackSequenceEvent::Expression { channel: 0, value: 92 },
            },
        ];

        let (events, _) = parse_sequence_events(&sequence, 1, 1, 0, true, &[], &mut tone_map);
        assert!(events.iter().any(|(_, event)| matches!(
            event,
            SmafEvent::MidiControlChange {
                channel: 0,
                control: 7,
                value: 72
            }
        )));
        assert!(!events
            .iter()
            .any(|(_, event)| matches!(event, SmafEvent::MidiControlChange { channel: 0, control: 11, .. })));
    }

    #[test]
    fn hps_melody_pitch_adds_base_offset_but_rhythm_does_not() {
        let mut melody_map = ToneMap::new();
        let melody = [channel_status(ChannelType::Melody)];
        melody_map.init_track(smaf::FormatType::HandyPhoneStandard, &melody, 0);
        assert_eq!(melody_map.map_note(0, 24), 60);

        let mut rhythm_map = ToneMap::new();
        let rhythm = [channel_status(ChannelType::Rhythm)];
        rhythm_map.init_track(smaf::FormatType::HandyPhoneStandard, &rhythm, 0);
        assert_eq!(rhythm_map.set_program(0, 38), (9, 0));
        assert_eq!(rhythm_map.map_note(0, 24), 38);
    }
}
