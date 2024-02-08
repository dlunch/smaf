use core::time::Duration;
use std::{cell::RefCell, env::args, fs};

use midir::{MidiOutput, MidiOutputConnection};
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};
use tokio::time::sleep;

use smaf::Smaf;
use smaf_player::{play_smaf, AudioBackend};

struct AudioBackendImpl {
    midi_out: RefCell<MidiOutputConnection>,
    sink: Sink,
}

impl AudioBackendImpl {
    pub fn new(midi_out: MidiOutputConnection, stream_handle: OutputStreamHandle) -> Self {
        let sink = Sink::try_new(&stream_handle).unwrap();
        Self {
            midi_out: RefCell::new(midi_out),
            sink,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl AudioBackend for AudioBackendImpl {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]) {
        let buffer = SamplesBuffer::new(channel as _, sampling_rate as _, wave_data);

        self.sink.append(buffer);
    }

    fn midi_note_on(&self, channel_id: u8, note: u8, velocity: u8) {
        println!("[{}] Note On: {} Velocity: {}", channel_id, note, velocity);
        self.midi_out.borrow_mut().send(&[0x90 | channel_id, note, velocity]).unwrap();
    }

    fn midi_note_off(&self, channel_id: u8, note: u8, velocity: u8) {
        println!("[{}] Note Off: {} Velocity: {}", channel_id, note, velocity);
        self.midi_out.borrow_mut().send(&[0x80 | channel_id, note, velocity]).unwrap();
    }

    fn midi_control_change(&self, channel_id: u8, control: u8, value: u8) {
        println!("[{}] ControlChange: {} Value: {}", channel_id, control, value);
        self.midi_out.borrow_mut().send(&[0xB0 | channel_id, control, value]).unwrap()
    }

    fn midi_program_change(&self, channel_id: u8, program: u8) {
        println!("[{}] ProgramChange: {}", channel_id, program);
        self.midi_out.borrow_mut().send(&[0xC0 | channel_id, program]).unwrap()
    }

    async fn sleep(&self, duration: Duration) {
        sleep(duration).await
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let file = args().nth(1).expect("No file given");
    let data = fs::read(file).expect("Failed to read file");

    let smaf = Smaf::parse(&data).expect("Failed to parse file");

    let midi_out = MidiOutput::new("smaf_cli").unwrap();
    let midi_ports = midi_out.ports();
    let out_port = midi_ports.last().unwrap();
    let midi_out = midi_out.connect(out_port, "smaf_cli").unwrap();

    let (_output_stream, stream_handle) = OutputStream::try_default().unwrap();

    play_smaf(&smaf, &AudioBackendImpl::new(midi_out, stream_handle)).await
}
