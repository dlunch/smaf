type SmafResult<T> = anyhow::Result<T>;

pub struct SmafRawChunk<'a> {
    pub id: [u8; 4],
    pub size: u32,
    pub data: &'a [u8],
}

pub struct Smaf<'a> {
    pub chunks: Vec<SmafRawChunk<'a>>,
}

impl<'a> Smaf<'a> {
    pub fn new(file: &'a [u8]) -> SmafResult<Self> {
        let file_chunk = parse_chunks(file)?;
        anyhow::ensure!(file_chunk.len() == 1, "Invalid file chunk count");

        let file_chunk = &file_chunk[0];
        anyhow::ensure!(file_chunk.id == [b'M', b'M', b'M', b'D'], "Invalid file chunk id");
        anyhow::ensure!(file_chunk.size + 8 == file.len() as u32, "Invalid file size");

        let crc_base = file_chunk.data.len() - 2;
        let _crc = u16::from_be_bytes([file[crc_base], file[crc_base + 1]]);

        let chunks = parse_chunks(&file_chunk.data[..crc_base])?;

        Ok(Self { chunks })
    }
}

fn parse_chunks(data: &[u8]) -> SmafResult<Vec<SmafRawChunk<'_>>> {
    let mut chunks = Vec::new();
    let mut cursor = 0;
    while cursor < data.len() {
        let chunk_id = [data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]];
        let chunk_size = u32::from_be_bytes([data[cursor + 4], data[cursor + 5], data[cursor + 6], data[cursor + 7]]) as usize;
        let chunk_data = &data[cursor + 8..cursor + 8 + chunk_size];
        chunks.push(SmafRawChunk {
            id: chunk_id,
            size: chunk_size as u32,
            data: chunk_data,
        });
        cursor += 8 + chunk_size;
    }
    Ok(chunks)
}
