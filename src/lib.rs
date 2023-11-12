mod chunks;

use self::chunks::{content_info::ContentsInfoChunk, optional_data::OptionalDataChunk};

type SmafResult<T> = anyhow::Result<T>;

pub enum SmafChunk<'a> {
    ContentsInfo(ContentsInfoChunk<'a>), // CNTI
    OptionalData(OptionalDataChunk<'a>), // OPDA
    ScoreTrack(u8, &'a [u8]),            // MTRx
    PCMAudioTrack(u8, &'a [u8]),         // ATRx
}

pub struct Smaf<'a> {
    pub chunks: Vec<SmafChunk<'a>>,
}

impl<'a> Smaf<'a> {
    pub fn new(file: &'a [u8]) -> SmafResult<Self> {
        let file_chunk = chunks::parse_raw_chunks(file)?;
        anyhow::ensure!(file_chunk.len() == 1, "Invalid file chunk count");

        let file_chunk = &file_chunk[0];
        anyhow::ensure!(file_chunk.id == [b'M', b'M', b'M', b'D'], "Invalid file chunk id");
        anyhow::ensure!(file_chunk.size + 8 == file.len() as u32, "Invalid file size");

        let crc_base = file_chunk.data.len() - 2;
        let _crc = u16::from_be_bytes([file[crc_base], file[crc_base + 1]]);

        let raw_chunks = chunks::parse_raw_chunks(&file_chunk.data[..crc_base])?;

        let chunks = raw_chunks
            .into_iter()
            .map(|x| {
                Ok(match x.id {
                    [b'C', b'N', b'T', b'I'] => SmafChunk::ContentsInfo(ContentsInfoChunk::new(x.data)?),
                    [b'O', b'P', b'D', b'A'] => SmafChunk::OptionalData(OptionalDataChunk::new(x.data)?),
                    [b'M', b'T', b'R', _] => SmafChunk::ScoreTrack(x.id[3], x.data),
                    [b'A', b'T', b'R', _] => SmafChunk::PCMAudioTrack(x.id[3], x.data),
                    _ => anyhow::bail!("Invalid chunk id"),
                })
            })
            .collect::<SmafResult<Vec<_>>>()?;

        Ok(Self { chunks })
    }
}
