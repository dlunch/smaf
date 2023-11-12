pub mod content_info;
pub mod optional_data;

use crate::SmafResult;

pub struct SmafRawChunk<'a> {
    pub id: [u8; 4],
    pub size: u32,
    pub data: &'a [u8],
}

pub fn parse_raw_chunks(data: &[u8]) -> SmafResult<Vec<SmafRawChunk<'_>>> {
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
