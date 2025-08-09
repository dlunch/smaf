use core::time::Duration;
use std::{env::args, fs};

use midir::MidiOutput;
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use tokio::time::sleep;

use smaf_player::{parse_smaf, SmafEvent};

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let file = args().nth(1).expect("No file given");
    let data = fs::read(file).expect("Failed to read file");

    let midi_out = MidiOutput::new("smaf_cli").unwrap();
    let midi_ports = midi_out.ports();
    let out_port = midi_ports.last().unwrap();
    let mut midi_out = midi_out.connect(out_port, "smaf_cli").unwrap();

    let (_output_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let events = parse_smaf(&data);

    let mut now = 0;
    for (time, event) in events {
        sleep(Duration::from_millis((time - now) as u64)).await;

        match event {
            SmafEvent::Wave {
                channel,
                sampling_rate,
                data,
            } => {
                let buffer = SamplesBuffer::new(channel as _, sampling_rate as _, data);
                sink.append(buffer);
            }
            SmafEvent::MidiNoteOn { channel, note, velocity } => {
                midi_out.send(&[0x90 | channel, note, velocity]).unwrap();
            }
            SmafEvent::MidiNoteOff { channel, note, velocity } => {
                midi_out.send(&[0x80 | channel, note, velocity]).unwrap();
            }
            SmafEvent::MidiProgramChange { channel, program } => {
                midi_out.send(&[0xC0 | channel, program]).unwrap();
            }
            SmafEvent::MidiControlChange { channel, control, value } => {
                midi_out.send(&[0xB0 | channel, control, value]).unwrap();
            }
            SmafEvent::End => {}
        }

        now = time;
    }
}
