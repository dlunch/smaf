use core::time::Duration;
use std::{env::args, fs};

use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};
use tokio::time::sleep;

use smaf::Smaf;
use smaf_player::{play_smaf, AudioBackend};

struct AudioBackendImpl {
    sink: Sink,
}

impl AudioBackendImpl {
    pub fn new(stream_handle: OutputStreamHandle) -> Self {
        let sink = Sink::try_new(&stream_handle).unwrap();
        Self { sink }
    }
}

#[async_trait::async_trait]
impl AudioBackend for AudioBackendImpl {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]) {
        let buffer = SamplesBuffer::new(channel as _, sampling_rate as _, wave_data);

        self.sink.append(buffer);
    }

    fn midi_note_on(&self, _channel_id: u8, _note: u8, _velocity: u8) {}

    fn midi_note_off(&self, _channel_id: u8, _note: u8) {}

    fn midi_program_change(&self, _channel_id: u8, _program: u8) {}

    async fn sleep(&self, duration: Duration) {
        sleep(duration).await
    }
}

#[tokio::main]
pub async fn main() {
    let file = args().nth(1).expect("No file given");
    let data = fs::read(file).expect("Failed to read file");

    let smaf = Smaf::parse(&data).expect("Failed to parse file");

    let (_output_stream, stream_handle) = OutputStream::try_default().unwrap();

    play_smaf(&smaf, &AudioBackendImpl::new(stream_handle)).await;
}
