//! The application payload appended to the setup executable.
//!
//! Layout after the PE image (and before any Authenticode certificate table
//! a later signing step appends):
//!
//! ```text
//! [zlib stream] [0..=7 zero padding] [64-byte trailer]
//! trailer = "JJSETUP1" | u64 offset | u64 compressed_len | [32] sha256 | u64 raw_len
//! raw     = "JJPAYLD1" | u16 version_len | version | u32 count
//!           { u16 name_len | name | u64 size | [32] sha256 | bytes }*
//! ```
//!
//! Every length is bounded, every name is a plain file name, and both the
//! compressed stream and every file are SHA-256 verified before use.

use crate::sha::Sha256;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const TRAILER_MAGIC: &[u8; 8] = b"JJSETUP1";
const PAYLOAD_MAGIC: &[u8; 8] = b"JJPAYLD1";
const TRAILER_LEN: u64 = 64;
const MAX_COMPRESSED: u64 = 256 * 1024 * 1024;
const MAX_RAW: u64 = 512 * 1024 * 1024;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_FILES: u32 = 32;
const MAX_VERSION: usize = 64;

/// The files every Jawjack payload must contain.
pub const REQUIRED_FILES: [&str; 1] = ["Jawjack.exe"];

pub struct PayloadFile {
    pub name: String,
    pub sha256: [u8; 32],
    pub bytes: Vec<u8>,
}

pub struct Payload {
    pub version: String,
    pub files: Vec<PayloadFile>,
    /// File offset where the payload starts: the bytes before it are the
    /// plain setup executable, reused as the installed uninstaller.
    pub image_len: u64,
}

impl Payload {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.bytes.len() as u64).sum()
    }
}

#[derive(Debug)]
pub enum PayloadError {
    /// This executable carries no payload (for example a plain Cargo build).
    Missing,
    /// The payload is present but damaged or malformed.
    Damaged(&'static str),
    Io(std::io::Error),
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(
                f,
                "this setup file does not contain the Jawjack application"
            ),
            Self::Damaged(reason) => write!(f, "this setup file is damaged ({reason})"),
            Self::Io(error) => write!(f, "this setup file could not be read ({error})"),
        }
    }
}

impl From<std::io::Error> for PayloadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// End of the data a signing tool would hash: the Authenticode certificate
/// table (PE security directory), when present, is excluded.
fn content_end(file: &mut File, len: u64) -> Result<u64, PayloadError> {
    let mut header = [0u8; 64];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    if &header[..2] != b"MZ" {
        return Err(PayloadError::Damaged("not a Windows executable"));
    }
    let pe = u32::from_le_bytes(header[0x3c..0x40].try_into().unwrap()) as u64;
    if pe > 4096 || pe + 24 + 240 > len {
        return Err(PayloadError::Damaged("invalid executable header"));
    }
    let mut nt = [0u8; 24 + 240];
    file.seek(SeekFrom::Start(pe))?;
    file.read_exact(&mut nt)?;
    if &nt[..4] != b"PE\0\0" || u16::from_le_bytes([nt[24], nt[25]]) != 0x20b {
        return Err(PayloadError::Damaged("not an x64 executable"));
    }
    // PE32+ optional header: data directories start at offset 112; the
    // security directory is entry 4 (8 bytes each).
    let directory = 24 + 112 + 4 * 8;
    let offset = u32::from_le_bytes(nt[directory..directory + 4].try_into().unwrap()) as u64;
    let size = u32::from_le_bytes(nt[directory + 4..directory + 8].try_into().unwrap()) as u64;
    if offset != 0 && size != 0 {
        if offset.checked_add(size) != Some(len) || offset > len {
            return Err(PayloadError::Damaged("unexpected signature layout"));
        }
        Ok(offset)
    } else {
        Ok(len)
    }
}

/// Locate, verify and decode the payload appended to `executable`.
pub fn load(executable: &Path) -> Result<Payload, PayloadError> {
    let mut file = File::open(executable)?;
    let len = file.metadata()?.len();
    let end = content_end(&mut file, len)?;
    // A signing tool may pad the content to 8 bytes before its table.
    let mut trailer = [0u8; TRAILER_LEN as usize];
    let mut found = None;
    for padding in 0..8u64 {
        let Some(start) = end.checked_sub(TRAILER_LEN + padding) else {
            break;
        };
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut trailer)?;
        if &trailer[..8] == TRAILER_MAGIC {
            found = Some(start);
            break;
        }
    }
    let Some(trailer_start) = found else {
        return Err(PayloadError::Missing);
    };
    let field =
        |range: std::ops::Range<usize>| u64::from_le_bytes(trailer[range].try_into().unwrap());
    let offset = field(8..16);
    let compressed_len = field(16..24);
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&trailer[24..56]);
    let raw_len = field(56..64);
    if compressed_len == 0
        || compressed_len > MAX_COMPRESSED
        || raw_len > MAX_RAW
        || offset < 1024
        || offset
            .checked_add(compressed_len)
            .is_none_or(|stop| stop > trailer_start)
        || trailer_start - (offset + compressed_len) > 7
    {
        return Err(PayloadError::Damaged("invalid payload bounds"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut compressed = vec![0u8; compressed_len as usize];
    file.read_exact(&mut compressed)?;
    let mut hasher = Sha256::new().map_err(|_| PayloadError::Damaged("SHA-256 unavailable"))?;
    hasher.update(&compressed);
    if hasher
        .finish()
        .map_err(|_| PayloadError::Damaged("SHA-256 unavailable"))?
        != expected
    {
        return Err(PayloadError::Damaged("payload SHA-256 mismatch"));
    }
    let mut raw = Vec::with_capacity(raw_len as usize);
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .take(MAX_RAW + 1)
        .read_to_end(&mut raw)
        .map_err(|_| PayloadError::Damaged("payload could not be decompressed"))?;
    if raw.len() as u64 != raw_len {
        return Err(PayloadError::Damaged("payload size mismatch"));
    }
    let mut payload = parse(&raw)?;
    payload.image_len = offset;
    Ok(payload)
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], PayloadError> {
        if self.0.len() < count {
            return Err(PayloadError::Damaged("truncated payload"));
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }
    fn u16(&mut self) -> Result<u16, PayloadError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, PayloadError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, PayloadError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}

pub fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= MAX_VERSION
        && version
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit())
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
}

fn parse(raw: &[u8]) -> Result<Payload, PayloadError> {
    let mut cursor = Cursor(raw);
    if cursor.take(8)? != PAYLOAD_MAGIC {
        return Err(PayloadError::Damaged("unknown payload format"));
    }
    let version_len = cursor.u16()? as usize;
    let version = std::str::from_utf8(cursor.take(version_len)?)
        .ok()
        .filter(|version| valid_version(version))
        .ok_or(PayloadError::Damaged("invalid payload version"))?
        .to_owned();
    let count = cursor.u32()?;
    if count == 0 || count > MAX_FILES {
        return Err(PayloadError::Damaged("invalid payload file count"));
    }
    let mut files: Vec<PayloadFile> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name_len = cursor.u16()? as usize;
        let name = std::str::from_utf8(cursor.take(name_len)?)
            .ok()
            .filter(|name| valid_file_name(name))
            .ok_or(PayloadError::Damaged("invalid payload file name"))?
            .to_owned();
        if files
            .iter()
            .any(|file| file.name.eq_ignore_ascii_case(&name))
        {
            return Err(PayloadError::Damaged("duplicate payload file"));
        }
        let size = cursor.u64()?;
        if size > MAX_FILE {
            return Err(PayloadError::Damaged("payload file too large"));
        }
        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(cursor.take(32)?);
        let bytes = cursor.take(size as usize)?.to_vec();
        let mut hasher = Sha256::new().map_err(|_| PayloadError::Damaged("SHA-256 unavailable"))?;
        hasher.update(&bytes);
        if hasher
            .finish()
            .map_err(|_| PayloadError::Damaged("SHA-256 unavailable"))?
            != sha256
        {
            return Err(PayloadError::Damaged("payload file SHA-256 mismatch"));
        }
        files.push(PayloadFile {
            name,
            sha256,
            bytes,
        });
    }
    if !cursor.0.is_empty() {
        return Err(PayloadError::Damaged("trailing payload data"));
    }
    for required in REQUIRED_FILES {
        if !files.iter().any(|file| file.name == required) {
            return Err(PayloadError::Damaged(
                "payload is missing an application file",
            ));
        }
    }
    Ok(Payload {
        version,
        files,
        image_len: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(version: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut raw = PAYLOAD_MAGIC.to_vec();
        raw.extend((version.len() as u16).to_le_bytes());
        raw.extend(version.as_bytes());
        raw.extend((files.len() as u32).to_le_bytes());
        for (name, bytes) in files {
            raw.extend((name.len() as u16).to_le_bytes());
            raw.extend(name.as_bytes());
            raw.extend((bytes.len() as u64).to_le_bytes());
            let mut hasher = Sha256::new().unwrap();
            hasher.update(bytes);
            raw.extend(hasher.finish().unwrap());
            raw.extend(*bytes);
        }
        raw
    }

    fn complete() -> Vec<(&'static str, &'static [u8])> {
        REQUIRED_FILES
            .iter()
            .map(|name| (*name, &b"x"[..]))
            .collect()
    }

    #[test]
    fn parses_complete_payload() {
        let payload = parse(&encode("0.1.0-preview.2", &complete())).unwrap();
        assert_eq!(payload.version, "0.1.0-preview.2");
        assert_eq!(payload.files.len(), REQUIRED_FILES.len());
    }

    #[test]
    fn rejects_traversal_missing_files_and_bad_hashes() {
        let mut files = complete();
        files.push(("..\\evil.exe", b"x"));
        assert!(parse(&encode("0.1.0", &files)).is_err());
        assert!(parse(&encode("0.1.0", &complete()[1..])).is_err());
        let mut raw = encode("0.1.0", &complete());
        let last = raw.len() - 1;
        raw[last] ^= 1;
        assert!(parse(&raw).is_err());
        assert!(parse(&encode("../1", &complete())).is_err());
    }

    #[test]
    fn file_names_are_plain() {
        assert!(valid_file_name("Jawjack.exe"));
        for name in ["", ".hidden", "a/b", "a\\b", "c:x", "a b"] {
            assert!(!valid_file_name(name), "{name}");
        }
    }
}
