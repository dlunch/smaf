use smaf::{
    parse_handy_variable_number, parse_variable_number, BaseBit, Channel, FormatType, PCMAudioSequenceData, PCMAudioSequenceEvent,
    PCMAudioTrackChunk, PCMDataChunk, PcmWaveFormat, ScoreTrackChunk, ScoreTrackSequenceEvent, SequenceData, Smaf, SmafChunk, StreamWaveFormat,
};

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test_data/bell.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
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
fn test_wave_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test_data/wave.mmf");
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
    let data = include_bytes!("../../test_data/midi.mmf");
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
    data.extend_from_slice(&0u16.to_be_bytes());

    let file = Smaf::parse(&data)?;

    assert!(file.chunks.iter().any(|c| matches!(c, SmafChunk::ContentsInfo(_))));
    assert!(file.chunks.iter().any(|c| matches!(c, SmafChunk::Unknown(b"XXXX", _))));

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

    assert!(events
        .iter()
        .any(|e| matches!(e.event, ScoreTrackSequenceEvent::Expression { channel: 0, value: 0x37 })));
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

    assert!(events
        .iter()
        .any(|e| matches!(e.event, PCMAudioSequenceEvent::Expression { channel: 0, value: 0x37 })));
}

#[test]
fn test_pcm_short_pitch_bend() {
    // duration=0, first_byte=0x00, second_byte=0x13 (channel 0, event_type 0x13 = short pitch bend)
    // then EoS
    let seq = [0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00];
    let (_, events) = PCMAudioSequenceData::parse(&seq).unwrap();

    assert!(events
        .iter()
        .any(|e| matches!(e.event, PCMAudioSequenceEvent::PitchBend { channel: 0, value: 0x18 })));
}
