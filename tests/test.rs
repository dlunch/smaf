use smaf::Smaf;

#[test]
fn test_bell_load() -> anyhow::Result<()> {
    let data = include_bytes!("./test_data/bell.mmf");
    let file = Smaf::new(data)?;

    assert_eq!(file.chunks.len(), 3);
    assert_eq!(file.chunks[0].id, [b'C', b'N', b'T', b'I']);
    assert_eq!(file.chunks[1].id, [b'O', b'P', b'D', b'A']);
    assert_eq!(file.chunks[2].id, [b'M', b'T', b'R', 6]);

    Ok(())
}
