use core::time::Duration;
use std::{env::args, fs, num::NonZero};

use midir::{MidiOutput, MidiOutputConnection};
use rodio::{DeviceSinkBuilder, Player, buffer::SamplesBuffer, conversions::SampleTypeConverter};
use tokio::time::sleep;

use smaf_player::{SmafEvent, parse_smaf};

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let file = args().nth(1).expect("No file given");
    let data = fs::read(file).expect("Failed to read file");

    let midi_out = MidiOutput::new("smaf-cli").unwrap();
    let midi_ports = midi_out.ports();
    let out_port = midi_ports.last().unwrap();
    let mut midi_out = midi_out.connect(out_port, "smaf-cli").unwrap();

    let output = DeviceSinkBuilder::open_default_sink().unwrap();
    let player = Player::connect_new(output.mixer());

    let events = parse_smaf(&data);

    loop {
        play_events(&events, &mut midi_out, &player).await;
    }
}

async fn play_events(events: &[(usize, SmafEvent)], midi_out: &mut MidiOutputConnection, player: &Player) {
    let mut now = 0;
    for (time, event) in events {
        sleep(Duration::from_millis(time.saturating_sub(now) as u64)).await;

        match event {
            SmafEvent::Wave {
                channel,
                sampling_rate,
                data,
            } => {
                let samples = SampleTypeConverter::<_, f32>::new(data.iter().copied()).collect::<Vec<_>>();
                let buffer = SamplesBuffer::new(NonZero::new(*channel as u16).unwrap(), NonZero::new(*sampling_rate).unwrap(), samples);
                player.append(buffer);
            }
            SmafEvent::MidiNoteOn { channel, note, velocity } => {
                midi_out.send(&[0x90 | *channel, *note, *velocity]).unwrap();
            }
            SmafEvent::MidiNoteOff { channel, note, velocity } => {
                midi_out.send(&[0x80 | *channel, *note, *velocity]).unwrap();
            }
            SmafEvent::MidiProgramChange { channel, program } => {
                midi_out.send(&[0xC0 | *channel, *program]).unwrap();
            }
            SmafEvent::MidiControlChange { channel, control, value } => {
                midi_out.send(&[0xB0 | *channel, *control, *value]).unwrap();
            }
            SmafEvent::MidiPitchBend { channel, value } => {
                midi_out
                    .send(&[0xE0 | *channel, (*value & 0x7f) as u8, ((*value >> 7) & 0x7f) as u8])
                    .unwrap();
            }
            SmafEvent::MidiSysEx(data) => {
                midi_out.send(data).unwrap();
            }
            SmafEvent::End => {}
        }

        now = *time;
    }
}
