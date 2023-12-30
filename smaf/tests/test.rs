use smaf::{BaseBit, Channel, PcmDataChunk, PcmWaveFormat, ScoreTrackChunk, Smaf, SmafChunk, StreamWaveFormat};

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("../../test_data/bell.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(6, _)));

    if let SmafChunk::ScoreTrack(_, x) = &file.chunks[2] {
        assert_eq!(x.chunks.len(), 3);
        assert!(matches!(x.chunks[0], ScoreTrackChunk::SetupData(_)));
        assert!(matches!(x.chunks[1], ScoreTrackChunk::SequenceData(_)));
        assert!(matches!(x.chunks[2], ScoreTrackChunk::PcmData(_)));

        if let ScoreTrackChunk::PcmData(x) = &x.chunks[2] {
            assert_eq!(x.len(), 1);
            assert!(matches!(x[0], PcmDataChunk::WaveData(1, _)));

            let smaf::PcmDataChunk::WaveData(_, x) = &x[0];

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

        assert_eq!(x.chunks.len(), 12874);
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
        assert_eq!(x.chunks.len(), 2);
        assert!(matches!(x.chunks[0], ScoreTrackChunk::SetupData(_)));
        assert!(matches!(x.chunks[1], ScoreTrackChunk::SequenceData(_)));
    } else {
        panic!("Expected ScoreTrack chunk");
    }

    Ok(())
}
