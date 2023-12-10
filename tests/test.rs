use smaf::{ScoreTrackChunk, Smaf, SmafChunk};

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/bell.mmf");
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
            assert!(matches!(x[0], smaf::PcmDataChunk::WaveData(_, _)));
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
    let data = include_bytes!("./test_data/wave.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 2);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::PCMAudioTrack(0, _)));

    Ok(())
}

#[test]
fn test_midi_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/midi.mmf");
    let file = Smaf::parse(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(5, _)));

    Ok(())
}
