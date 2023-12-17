use std::{env::args, fs, thread};

use rodio::{buffer::SamplesBuffer, OutputStream, Sink, Source};

use smaf::Smaf;
use smaf_player::{play_smaf, AudioBackend};

struct AudioBackendImpl;

impl AudioBackend for AudioBackendImpl {
    fn play_wave(&self, channel: u8, sampling_rate: u32, wave_data: &[i16]) {
        let buffer = SamplesBuffer::new(channel as _, sampling_rate as _, wave_data);
        let duration = buffer.total_duration().unwrap();

        let (_output_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.append(buffer);

        thread::sleep(duration);
    }
}

pub fn main() {
    let file = args().nth(1).expect("No file given");
    let data = fs::read(file).expect("Failed to read file");

    let smaf = Smaf::parse(&data).expect("Failed to parse file");
    play_smaf(&smaf, &AudioBackendImpl);
}
