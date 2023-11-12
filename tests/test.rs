use smaf::{Smaf, SmafChunk};

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/bell.mmf");
    let file = Smaf::new(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(6, _)));

    Ok(())
}

#[test]
fn test_wave_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/wave.mmf");
    let file = Smaf::new(data)?;

    assert_eq!(file.chunks.len(), 2);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::PCMAudioTrack(0, _)));

    Ok(())
}

#[test]
fn test_midi_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/midi.mmf");
    let file = Smaf::new(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert!(matches!(file.chunks[0], SmafChunk::ContentsInfo(_)));
    assert!(matches!(file.chunks[1], SmafChunk::OptionalData(_)));
    assert!(matches!(file.chunks[2], SmafChunk::ScoreTrack(5, _)));

    Ok(())
}
