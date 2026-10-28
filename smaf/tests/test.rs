use smaf::{
    BaseBit, Channel, FormatType, PCMAudioSequenceData, PCMAudioSequenceEvent, PCMAudioTrackChunk, PCMDataChunk, PcmWaveFormat, ScoreTrackChunk,
    ScoreTrackSequenceEvent, SequenceData, Smaf, SmafChunk, StreamWaveFormat, parse_handy_variable_number, parse_variable_number,
};

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test-data/bell.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 3);
    let SmafChunk::ContentsInfo(info) = &file.chunks[0] else {
        panic!("Expected ContentsInfo chunk");
    };
    assert_eq!(
        [
            info.content_class,
            info.content_type,
            info.content_code_type,
            info.copy_status,
            info.copy_counts
        ],
        [0x00, 0x34, 0x01, 0xf8, 0x00]
    );
    assert!(info.option.is_empty());
    let SmafChunk::OptionalData(optional) = &file.chunks[1] else {
        panic!("Expected OptionalData chunk");
    };
    assert_eq!(optional.raw, &data[29..82]);
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(6, _)));

    if let SmafChunk::ScoreTrack(_, x) = &file.chunks[2] {
        assert_eq!(x.format_type, FormatType::MobileStandardNoCompress);

        assert_eq!(x.chunks.len(), 3);
        assert!(matches!(x.chunks[0], ScoreTrackChunk::SetupData(_)));
        assert!(matches!(x.chunks[1], ScoreTrackChunk::SequenceData(_)));
        assert!(matches!(x.chunks[2], ScoreTrackChunk::PCMData(_)));

        if let ScoreTrackChunk::PCMData(x) = &x.chunks[2] {
            assert_eq!(x.len(), 1);
            assert!(matches!(x[0], PCMDataChunk::WaveData(1, _)));

            let smaf::PCMDataChunk::WaveData(_, x) = &x[0];

            assert_eq!(x.channel, Channel::Mono);
            assert_eq!(x.format, StreamWaveFormat::YamahaADPCM);
            assert_eq!(x.base_bit, BaseBit::Bit4);
            assert_eq!(x.sampling_freq, 22050);

            assert_eq!(x.wave_data.len(), 367616);
        } else {
            panic!("Expected PcmData chunk");
        }
    } else {
        panic!("Expected ScoreTrack chunk");
    }

    Ok(())
}

#[test]
fn test_score_track_header_formats() {
    for format in 0..=3 {
        let mut track = vec![format, 0, 0x11, 2];
        track.resize(if format == 0 { 6 } else { 20 }, 0);
        let mut data = b"MMMD".to_vec();
        data.extend_from_slice(&(track.len() as u32 + 10).to_be_bytes());
        data.extend_from_slice(b"MTR\x00");
        data.extend_from_slice(&(track.len() as u32).to_be_bytes());
        data.extend_from_slice(&track);
        data.extend_from_slice(&[0, 0]);

        if format == 3 {
            assert!(Smaf::parse(&data).is_err());
            continue;
        }

        let file = Smaf::parse(&data).unwrap();
        let SmafChunk::ScoreTrack(0, track) = &file.chunks[0] else {
            panic!("Expected ScoreTrack chunk");
        };
        assert_eq!(track.format_type as u8, format);
        assert_eq!(track.sequence_type, 0);
        assert_eq!(track.timebase_d, 20);
        assert_eq!(track.timebase_g, 4);
        assert_eq!(track.channel_status.len(), if format == 0 { 4 } else { 16 });
        assert!(track.chunks.is_empty());
    }
}

#[test]
fn test_wave_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test-data/wave.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 2);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::PCMAudioTrack(0, _)));

    if let SmafChunk::PCMAudioTrack(_, x) = &file.chunks[1] {
        assert_eq!(x.format_type, 0);
        assert_eq!(x.sequence_type, 0);
        assert_eq!(x.channel, Channel::Mono);
        assert_eq!(x.format, PcmWaveFormat::Adpcm);
        assert_eq!(x.sampling_freq, 8000);
        assert_eq!(x.base_bit, BaseBit::Bit4);
        assert_eq!(x.timebase_d, 4);
        assert_eq!(x.timebase_g, 4);

        assert_eq!(x.chunks.len(), 3);

        assert!(matches!(x.chunks[0], PCMAudioTrackChunk::SeekAndPhraseInfo(_)));
        assert!(matches!(x.chunks[1], PCMAudioTrackChunk::SequenceData(_)));
        assert!(matches!(x.chunks[2], PCMAudioTrackChunk::WaveData(1, _)));
    } else {
        panic!("Expected PCMAudioTrack chunk");
    }

    Ok(())
}

#[test]
fn test_midi_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test-data/midi.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(5, _)));

    if let SmafChunk::ScoreTrack(_, x) = &file.chunks[2] {
        assert_eq!(x.format_type, FormatType::MobileStandardNoCompress);

        assert_eq!(x.chunks.len(), 2);
        assert!(matches!(x.chunks[0], ScoreTrackChunk::SetupData(_)));
        assert!(matches!(x.chunks[1], ScoreTrackChunk::SequenceData(_)));
    } else {
        panic!("Expected ScoreTrack chunk");
    }

    Ok(())
}

#[test]
fn test_unknown_top_level_chunk_is_skipped() -> anyhow::Result<()> {
    // Build a minimal MMMD file: CNTI chunk + unknown "XXXX" chunk
    let mut data = Vec::new();
    // MMMD magic
    data.extend_from_slice(b"MMMD");
    // Total length (excluding magic + length fields = 8 bytes): CNTI(13) + XXXX(12) + CRC(2) = 27
    data.extend_from_slice(&27u32.to_be_bytes());
    // CNTI chunk: tag + length(5) + data(5 bytes: 5 u8 fields, empty rest)
    data.extend_from_slice(b"CNTI");
    data.extend_from_slice(&5u32.to_be_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    // Unknown chunk: tag + length(4) + data(4)
    data.extend_from_slice(b"XXXX");
    data.extend_from_slice(&4u32.to_be_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    // CRC
    data.extend_from_slice(&0x1234u16.to_be_bytes());

    let file = Smaf::parse(&data)?;

    assert!(file.chunks.iter().any(|c| matches!(c, SmafChunk::ContentsInfo(_))));
    assert!(file.chunks.iter().any(|c| matches!(c, SmafChunk::Unknown(b"XXXX", _))));
    assert_eq!(file.crc, 0x1234);

    for padding in [6, 14, 64] {
        let mut buffered = data.clone();
        buffered.resize(data.len() + padding, 0);
        let file = Smaf::parse(&buffered)?;
        assert_eq!(file.chunks.len(), 2);
        assert_eq!(file.crc, 0x1234);
    }

    assert!(Smaf::parse(&data[..data.len() - 1]).is_err());
    data[4..8].copy_from_slice(&1u32.to_be_bytes());
    assert!(Smaf::parse(&data).is_err());

    Ok(())
}

#[test]
fn test_mobile_reserved_status_bytes_are_ignored() {
    // duration=0, status=0xA5 (reserved), 2 data bytes, then 0xFF 0x2F 0x00 (end)
    let seq = [0x00, 0xA5, 0x12, 0x34, 0x00, 0xFF, 0x2F, 0x00];
    let (_, events) = SequenceData::parse_mobile(&seq).unwrap();

    // The reserved 0xA5 should produce a Nop, and the stream should end cleanly
    assert!(events.iter().all(|e| matches!(e.event, ScoreTrackSequenceEvent::Nop)));
}

#[test]
fn test_handy_eos_with_short_data_does_not_panic() {
    // Trailing data shorter than 4 bytes should not cause an out-of-bounds panic
    let seq = [0x00, 0x01, 0x00];
    let _ = SequenceData::parse_handy(&seq);
}

#[test]
fn test_handy_variable_number_single_byte() {
    let (_, val) = parse_handy_variable_number(&[0x42]).unwrap();
    assert_eq!(val, 0x42);
}

#[test]
fn test_handy_variable_number_two_byte() {
    let (_, val) = parse_handy_variable_number(&[0x82, 0x34]).unwrap();
    assert_eq!(val, ((0x02 + 1) << 7) | 0x34);
}

#[test]
fn test_mobile_variable_number_single_byte_fast_path() {
    let (_, val) = parse_variable_number(&[0x42]).unwrap();
    assert_eq!(val, 0x42);
}

#[test]
fn test_hps_short_pitch_bend() {
    // duration=0, status=0x00, next_byte=0x13 (channel 0, event_type 0x13 = short pitch bend)
    // then EoS
    let seq = [0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = SequenceData::parse_handy(&seq).unwrap();

    assert!(events.iter().any(|e| matches!(
        e.event,
        ScoreTrackSequenceEvent::PitchBend { channel: 0, value: v } if v == ((0x13 - 0x10) * 16384 / 16)
    )));
}

#[test]
fn test_hps_short_expression() {
    // duration=0, status=0x00, next_byte=0x05 (channel 0, event_type 0x05 = short expression)
    // then EoS
    let seq = [0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = SequenceData::parse_handy(&seq).unwrap();

    assert!(
        events
            .iter()
            .any(|e| matches!(e.event, ScoreTrackSequenceEvent::Expression { channel: 0, value: 0x37 }))
    );
}

#[test]
fn test_hps_note_has_no_velocity() {
    // duration=0, status=0x49 (channel 1, octave 0, voice 9 = note 9), gate_time=0
    // then EoS
    let seq = [0x00, 0x49, 0x00, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = SequenceData::parse_handy(&seq).unwrap();

    assert!(events.iter().any(|e| matches!(
        e.event,
        ScoreTrackSequenceEvent::NoteMessage {
            channel: 1,
            note: 9,
            velocity: None,
            ..
        }
    )));
}

#[test]
fn test_softbank_exclusive_uses_length_prefix() {
    // duration=0, status=0xFF, next_byte=0xF0 (exclusive), length=3, data=[0x41,0x42,0x43]
    // then EoS
    let seq = [0x00, 0xFF, 0xF0, 0x03, 0x41, 0x42, 0x43, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = SequenceData::parse_softbank(&seq).unwrap();

    assert!(events.iter().any(|e| matches!(
        e.event,
        ScoreTrackSequenceEvent::Exclusive(ref d) if d == &[0x41, 0x42, 0x43]
    )));
}

#[test]
fn test_hps_exclusive_uses_f7_terminator() {
    // duration=0, status=0xFF, next_byte=0xF0 (exclusive), data until 0xF7
    let seq = [0x00, 0xFF, 0xF0, 0x41, 0x42, 0xF7, 0x00, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = SequenceData::parse_handy(&seq).unwrap();

    assert!(events.iter().any(|e| matches!(
        e.event,
        ScoreTrackSequenceEvent::Exclusive(ref d) if d == &[0x41, 0x42]
    )));
}

#[test]
fn test_pcm_short_expression() {
    // duration=0, first_byte=0x00, second_byte=0x05 (channel 0, event_type 0x05 = short expression)
    // then EoS
    let seq = [0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = PCMAudioSequenceData::parse(&seq).unwrap();

    assert!(
        events
            .iter()
            .any(|e| matches!(e.event, PCMAudioSequenceEvent::Expression { channel: 0, value: 0x37 }))
    );
}

#[test]
fn test_pcm_short_pitch_bend() {
    // duration=0, first_byte=0x00, second_byte=0x13 (channel 0, event_type 0x13 = short pitch bend)
    // then EoS
    let seq = [0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = PCMAudioSequenceData::parse(&seq).unwrap();

    assert!(
        events
            .iter()
            .any(|e| matches!(e.event, PCMAudioSequenceEvent::PitchBend { channel: 0, value: 0x18 }))
    );
}
