/// Wire protocol for encoding data into QR code frames.
///
/// Each QR code contains a binary payload with the following structure:
///
/// **Header frame (frame_index == 0)**:
/// ```text
/// [magic: 4 bytes "QRDV"]
/// [version: 1 byte]
/// [flags: 1 byte]          // bit 0 = encrypted, bit 1 = compressed
/// [total_frames: 4 bytes LE]
/// [original_size: 8 bytes LE]
/// [checksum: 4 bytes]      // CRC32 of original (unencrypted, uncompressed) data
/// [filename_len: 2 bytes LE]
/// [filename: N bytes]
/// [salt: 16 bytes]         // only if encrypted
/// [nonce: 12 bytes]        // only if encrypted
/// ```
///
/// **Data frame (frame_index > 0)**:
/// ```text
/// [frame_index: 4 bytes LE]
/// [chunk_crc: 4 bytes]     // CRC32 of this chunk's data
/// [data: remaining bytes]
/// ```

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use anyhow::{Result, bail, Context};

pub const MAGIC: &[u8; 4] = b"QRDV";
pub const VERSION: u8 = 1;

pub const FLAG_ENCRYPTED: u8 = 0x01;
pub const FLAG_COMPRESSED: u8 = 0x02;

#[derive(Debug, Clone)]
pub struct Header {
    pub flags: u8,
    pub total_frames: u32,
    pub original_size: u64,
    pub checksum: u32,
    pub filename: String,
    pub salt: Option<[u8; 16]>,
    pub nonce: Option<[u8; 12]>,
}

impl Header {
    pub fn is_encrypted(&self) -> bool {
        self.flags & FLAG_ENCRYPTED != 0
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.write_all(MAGIC)?;
        buf.write_u8(VERSION)?;
        buf.write_u8(self.flags)?;
        buf.write_u32::<LittleEndian>(self.total_frames)?;
        buf.write_u64::<LittleEndian>(self.original_size)?;
        buf.write_u32::<LittleEndian>(self.checksum)?;

        let filename_bytes = self.filename.as_bytes();
        buf.write_u16::<LittleEndian>(filename_bytes.len() as u16)?;
        buf.write_all(filename_bytes)?;

        if self.is_encrypted() {
            if let (Some(salt), Some(nonce)) = (&self.salt, &self.nonce) {
                buf.write_all(salt)?;
                buf.write_all(nonce)?;
            } else {
                bail!("Encrypted flag set but salt/nonce missing");
            }
        }

        Ok(buf)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic).context("Reading magic bytes")?;
        if &magic != MAGIC {
            bail!("Invalid magic bytes: expected QRDV, got {:?}", magic);
        }

        let version = cursor.read_u8().context("Reading version")?;
        if version != VERSION {
            bail!("Unsupported version: {}", version);
        }

        let flags = cursor.read_u8().context("Reading flags")?;
        let total_frames = cursor.read_u32::<LittleEndian>().context("Reading total_frames")?;
        let original_size = cursor.read_u64::<LittleEndian>().context("Reading original_size")?;
        let checksum = cursor.read_u32::<LittleEndian>().context("Reading checksum")?;

        let filename_len = cursor.read_u16::<LittleEndian>().context("Reading filename_len")? as usize;
        let mut filename_bytes = vec![0u8; filename_len];
        cursor.read_exact(&mut filename_bytes).context("Reading filename")?;
        let filename = String::from_utf8(filename_bytes).context("Filename is not valid UTF-8")?;

        let (salt, nonce) = if flags & FLAG_ENCRYPTED != 0 {
            let mut salt = [0u8; 16];
            let mut nonce = [0u8; 12];
            cursor.read_exact(&mut salt).context("Reading salt")?;
            cursor.read_exact(&mut nonce).context("Reading nonce")?;
            (Some(salt), Some(nonce))
        } else {
            (None, None)
        };

        Ok(Header {
            flags,
            total_frames,
            original_size,
            checksum,
            filename,
            salt,
            nonce,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DataFrame {
    pub frame_index: u32,
    pub chunk_crc: u32,
    pub data: Vec<u8>,
}

impl DataFrame {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(8 + self.data.len());
        buf.write_u32::<LittleEndian>(self.frame_index)?;
        buf.write_u32::<LittleEndian>(self.chunk_crc)?;
        buf.write_all(&self.data)?;
        Ok(buf)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        let frame_index = cursor.read_u32::<LittleEndian>().context("Reading frame_index")?;
        let chunk_crc = cursor.read_u32::<LittleEndian>().context("Reading chunk_crc")?;
        let mut payload = Vec::new();
        cursor.read_to_end(&mut payload)?;
        Ok(DataFrame {
            frame_index,
            chunk_crc,
            data: payload,
        })
    }
}
