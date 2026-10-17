//! Encrypted backup codec. Callers supply filtered, consistent snapshots without credentials.
//! Callers control evidence selection, disk-space measurement, preview approval, and live restore.
//!
//! Version 1 has a 60-byte header: magic, schema version, entry count, restored size, manifest size, salt, and nonce prefix.
//! All integers use little-endian encoding. The salt and nonce prefix each contain 16 random bytes.
//! Each encrypted frame authenticates the complete header, its index, plaintext length, and final-frame flag.
//! Its nonce joins the random prefix with the frame's 64-bit index. Encoded frames never exceed 1 MiB.
//! The encrypted payload starts with manifest entries: name length, file size, and name bytes. File contents follow in manifest order.
//! The limits are 10,000 files and 5 GiB of restored file bytes. Database rows do not count as files.

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use std::{
    collections::HashSet,
    fmt,
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"OUARCH01";
const HEADER_SIZE: usize = 60;
const FRAME_SIZE: usize = 13;
const TAG_SIZE: usize = 16;
const CHUNK_SIZE: usize = 1024 * 1024 - FRAME_SIZE - TAG_SIZE;
const MAX_RESTORED_SIZE: u64 = 5 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_NAME_SIZE: usize = 240;
const MAX_MANIFEST_SIZE: usize = MAX_ENTRIES * (10 + MAX_NAME_SIZE);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub schema_version: u32,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupError {
    InvalidPassword,
    InvalidManifest,
    LimitExceeded,
    InsufficientSpace,
    InvalidArchive,
    AuthenticationFailed,
    Io,
    RandomnessUnavailable,
    KeyDerivationFailed,
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPassword => "Use a backup password between 1 and 1024 bytes.",
            Self::InvalidManifest => "The backup manifest is invalid.",
            Self::LimitExceeded => "The backup exceeds the allowed file count or restored size.",
            Self::InsufficientSpace => {
                "The available disk space is insufficient for this backup operation."
            }
            Self::InvalidArchive => "The archive structure is invalid or incomplete.",
            Self::AuthenticationFailed => {
                "The archive could not be authenticated. Check its password and contents."
            }
            Self::Io => "The backup operation could not read or write its data.",
            Self::RandomnessUnavailable => "Secure random bytes are unavailable.",
            Self::KeyDerivationFailed => "The backup encryption key could not be derived.",
        })
    }
}

impl std::error::Error for BackupError {}

struct Header {
    bytes: [u8; HEADER_SIZE],
    schema_version: u32,
    entry_count: usize,
    restored_size: u64,
    manifest_size: usize,
}

impl Header {
    fn payload_size(&self) -> u64 {
        self.restored_size + self.manifest_size as u64
    }

    fn encoded_size(&self) -> u64 {
        let payload_size = self.payload_size();
        let chunks = payload_size.div_ceil(CHUNK_SIZE as u64);
        HEADER_SIZE as u64 + payload_size + chunks * (FRAME_SIZE + TAG_SIZE) as u64
    }

    fn parse(bytes: [u8; HEADER_SIZE]) -> Result<Self, BackupError> {
        if &bytes[..8] != MAGIC {
            return Err(BackupError::InvalidArchive);
        }
        let schema_version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let entry_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let restored_size = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let manifest_size = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        if entry_count > MAX_ENTRIES
            || restored_size > MAX_RESTORED_SIZE
            || manifest_size > MAX_MANIFEST_SIZE
        {
            return Err(BackupError::LimitExceeded);
        }
        if schema_version == 0 || entry_count == 0 || manifest_size < entry_count * 11 {
            return Err(BackupError::InvalidArchive);
        }
        Ok(Self {
            bytes,
            schema_version,
            entry_count,
            restored_size,
            manifest_size,
        })
    }
}

/// Return the complete encrypted archive size for a validated manifest, without key derivation or file operations.
pub fn encoded_size(manifest: &Manifest) -> Result<u64, BackupError> {
    let manifest_bytes = encode_manifest(manifest)?;
    Ok(manifest_header(manifest, manifest_bytes.len())?.encoded_size())
}

fn manifest_header(manifest: &Manifest, manifest_size: usize) -> Result<Header, BackupError> {
    let mut bytes = [0u8; HEADER_SIZE];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..12].copy_from_slice(&manifest.schema_version.to_le_bytes());
    bytes[12..16].copy_from_slice(&(manifest.entries.len() as u32).to_le_bytes());
    let restored_size = manifest.entries.iter().map(|entry| entry.size).sum::<u64>();
    bytes[16..24].copy_from_slice(&restored_size.to_le_bytes());
    bytes[24..28].copy_from_slice(&(manifest_size as u32).to_le_bytes());
    Header::parse(bytes)
}

/// The source contains entry bytes in manifest order, without file headers.
/// The caller must discard partial output after an error and verify the completed archive before publication.
pub fn encrypt<R: Read, W: Write>(
    manifest: &Manifest,
    source: &mut R,
    destination: &mut W,
    password: &[u8],
    available_space: u64,
) -> Result<(), BackupError> {
    let manifest_bytes = encode_manifest(manifest)?;
    let mut header = manifest_header(manifest, manifest_bytes.len())?;
    if header.encoded_size() > available_space {
        return Err(BackupError::InsufficientSpace);
    }
    getrandom::fill(&mut header.bytes[28..]).map_err(|_| BackupError::RandomnessUnavailable)?;
    let key = derive_key(password, &header.bytes[28..44])?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| BackupError::KeyDerivationFailed)?;
    destination
        .write_all(&header.bytes)
        .map_err(|_| BackupError::Io)?;
    let mut input = Cursor::new(manifest_bytes.as_slice()).chain(source);
    let mut remaining = header.payload_size();
    let mut index = 0u64;
    let mut plaintext = Zeroizing::new(vec![0u8; CHUNK_SIZE]);
    while remaining > 0 {
        let size = remaining.min(CHUNK_SIZE as u64) as usize;
        read_exact(&mut input, &mut plaintext[..size])?;
        let frame = frame(index, size, remaining == size as u64);
        let aad = additional_data(&header.bytes, &frame);
        let encrypted = cipher
            .encrypt(
                &nonce(&header, index),
                Payload {
                    msg: &plaintext[..size],
                    aad: &aad,
                },
            )
            .map_err(|_| BackupError::AuthenticationFailed)?;
        destination
            .write_all(&frame)
            .and_then(|()| destination.write_all(&encrypted))
            .map_err(|_| BackupError::Io)?;
        remaining -= size as u64;
        index += 1;
    }
    require_end(&mut input)?;
    destination.flush().map_err(|_| BackupError::Io)
}

/// Only fully authenticated archives produce this value. Its temporary file contains ciphertext.
pub struct VerifiedArchive {
    manifest: Manifest,
    encrypted: File,
    key: Zeroizing<[u8; 32]>,
    header: Header,
}

impl VerifiedArchive {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Copy entries in manifest order to a staging destination after preview approval.
    /// This method does not replace a live database or create archive paths.
    pub fn copy_payload<W: Write>(&mut self, destination: &mut W) -> Result<u64, BackupError> {
        self.encrypted
            .seek(SeekFrom::Start(HEADER_SIZE as u64))
            .map_err(|_| BackupError::Io)?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| BackupError::KeyDerivationFailed)?;
        let mut remaining = self.header.payload_size();
        let mut manifest_remaining = self.header.manifest_size;
        let mut index = 0u64;
        while remaining > 0 {
            let (plaintext, _) =
                read_chunk(&mut self.encrypted, &cipher, &self.header, index, remaining)?;
            let skip = manifest_remaining.min(plaintext.len());
            destination
                .write_all(&plaintext[skip..])
                .map_err(|_| BackupError::Io)?;
            manifest_remaining -= skip;
            remaining -= plaintext.len() as u64;
            index += 1;
        }
        require_end(&mut self.encrypted)?;
        destination.flush().map_err(|_| BackupError::Io)?;
        Ok(self.header.restored_size)
    }
}

/// Reserve checkpoint space before supplying this operation's available disk-space budget.
/// The budget covers the encrypted staging file plus the restored file bytes.
/// Use a private application directory on the measured local destination filesystem.
/// No plaintext reaches the caller before every chunk and the complete manifest pass authentication.
pub fn prepare_restore<R: Read>(
    source: &mut R,
    password: &[u8],
    staging_directory: &Path,
    available_space: u64,
) -> Result<VerifiedArchive, BackupError> {
    let mut bytes = [0u8; HEADER_SIZE];
    read_exact(source, &mut bytes)?;
    let header = Header::parse(bytes)?;
    if header.encoded_size() + header.restored_size > available_space {
        return Err(BackupError::InsufficientSpace);
    }
    let key = derive_key(password, &header.bytes[28..44])?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| BackupError::KeyDerivationFailed)?;
    let mut encrypted = tempfile::tempfile_in(staging_directory).map_err(|_| BackupError::Io)?;
    encrypted
        .write_all(&header.bytes)
        .map_err(|_| BackupError::Io)?;
    let mut manifest_bytes = Zeroizing::new(Vec::with_capacity(header.manifest_size));
    let mut remaining = header.payload_size();
    let mut index = 0u64;
    while remaining > 0 {
        let (plaintext, frame_bytes) = read_chunk(source, &cipher, &header, index, remaining)?;
        let needed = (header.manifest_size - manifest_bytes.len()).min(plaintext.len());
        manifest_bytes.extend_from_slice(&plaintext[..needed]);
        encrypted
            .write_all(&frame_bytes)
            .map_err(|_| BackupError::Io)?;
        remaining -= plaintext.len() as u64;
        index += 1;
    }
    require_end(source)?;
    let manifest = decode_manifest(&header, &manifest_bytes)?;
    Ok(VerifiedArchive {
        manifest,
        encrypted,
        key,
        header,
    })
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, BackupError> {
    if password.is_empty() || password.len() > 1024 {
        return Err(BackupError::InvalidPassword);
    }
    // Version 1 fixes Argon2id v1.3 at 64 MiB, three passes, and one lane.
    // Untrusted archives cannot select an unbounded KDF memory or CPU cost.
    let params =
        Params::new(64 * 1024, 3, 1, Some(32)).map_err(|_| BackupError::KeyDerivationFailed)?;
    // Wipe caller-owned KDF memory. The library's allocating helper only deallocates that memory.
    let mut memory = Zeroizing::new(Vec::new());
    memory
        .try_reserve_exact(params.block_count())
        .map_err(|_| BackupError::KeyDerivationFailed)?;
    memory.resize_with(params.block_count(), Block::default);
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(password, salt, key.as_mut(), memory.as_mut_slice())
        .map_err(|_| BackupError::KeyDerivationFailed)?;
    Ok(key)
}

fn frame(index: u64, size: usize, final_chunk: bool) -> [u8; FRAME_SIZE] {
    let mut result = [0u8; FRAME_SIZE];
    result[..8].copy_from_slice(&index.to_le_bytes());
    result[8..12].copy_from_slice(&(size as u32).to_le_bytes());
    result[12] = u8::from(final_chunk);
    result
}

fn nonce(header: &Header, index: u64) -> XNonce {
    let mut bytes = [0u8; 24];
    bytes[..16].copy_from_slice(&header.bytes[44..60]);
    bytes[16..].copy_from_slice(&index.to_le_bytes());
    bytes.into()
}

fn additional_data(
    header: &[u8; HEADER_SIZE],
    frame: &[u8; FRAME_SIZE],
) -> [u8; HEADER_SIZE + FRAME_SIZE] {
    let mut aad = [0u8; HEADER_SIZE + FRAME_SIZE];
    aad[..HEADER_SIZE].copy_from_slice(header);
    aad[HEADER_SIZE..].copy_from_slice(frame);
    aad
}

fn read_chunk<R: Read>(
    source: &mut R,
    cipher: &XChaCha20Poly1305,
    header: &Header,
    index: u64,
    remaining: u64,
) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>), BackupError> {
    let size = remaining.min(CHUNK_SIZE as u64) as usize;
    let expected_frame = frame(index, size, remaining == size as u64);
    let mut frame_bytes = [0u8; FRAME_SIZE];
    read_exact(source, &mut frame_bytes)?;
    if frame_bytes != expected_frame {
        return Err(BackupError::InvalidArchive);
    }
    let mut encrypted = vec![0u8; size + TAG_SIZE];
    read_exact(source, &mut encrypted)?;
    let aad = additional_data(&header.bytes, &frame_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &nonce(header, index),
                Payload {
                    msg: &encrypted,
                    aad: &aad,
                },
            )
            .map_err(|_| BackupError::AuthenticationFailed)?,
    );
    let mut encoded = Vec::with_capacity(FRAME_SIZE + encrypted.len());
    encoded.extend_from_slice(&frame_bytes);
    encoded.extend_from_slice(&encrypted);
    Ok((plaintext, encoded))
}

fn encode_manifest(manifest: &Manifest) -> Result<Zeroizing<Vec<u8>>, BackupError> {
    if manifest.schema_version == 0 || manifest.entries.is_empty() {
        return Err(BackupError::InvalidManifest);
    }
    if manifest.entries.len() > MAX_ENTRIES {
        return Err(BackupError::LimitExceeded);
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut bytes = Zeroizing::new(Vec::new());
    for entry in &manifest.entries {
        if !valid_name(&entry.name) || !seen.insert(entry.name.as_str()) {
            return Err(BackupError::InvalidManifest);
        }
        total = total
            .checked_add(entry.size)
            .ok_or(BackupError::LimitExceeded)?;
        if total > MAX_RESTORED_SIZE {
            return Err(BackupError::LimitExceeded);
        }
        bytes.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&entry.size.to_le_bytes());
        bytes.extend_from_slice(entry.name.as_bytes());
    }
    Ok(bytes)
}

fn decode_manifest(header: &Header, bytes: &[u8]) -> Result<Manifest, BackupError> {
    let mut cursor = Cursor::new(bytes);
    let mut entries = Vec::with_capacity(header.entry_count);
    for _ in 0..header.entry_count {
        let mut name_size = [0u8; 2];
        let mut entry_size = [0u8; 8];
        read_exact(&mut cursor, &mut name_size)?;
        read_exact(&mut cursor, &mut entry_size)?;
        let name_size = u16::from_le_bytes(name_size) as usize;
        if name_size == 0 || name_size > MAX_NAME_SIZE {
            return Err(BackupError::InvalidManifest);
        }
        let mut name = vec![0u8; name_size];
        read_exact(&mut cursor, &mut name)?;
        entries.push(Entry {
            name: String::from_utf8(name).map_err(|_| BackupError::InvalidManifest)?,
            size: u64::from_le_bytes(entry_size),
        });
    }
    require_end(&mut cursor)?;
    let manifest = Manifest {
        schema_version: header.schema_version,
        entries,
    };
    encode_manifest(&manifest)?;
    if manifest.entries.iter().map(|entry| entry.size).sum::<u64>() != header.restored_size {
        return Err(BackupError::InvalidManifest);
    }
    Ok(manifest)
}

fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_SIZE {
        return false;
    }
    name.split('/').all(|part| {
        if part.is_empty() || part == "." || part == ".." || part.ends_with('.') {
            return false;
        }
        let stem = part.split('.').next().unwrap_or("");
        if matches!(stem, "con" | "prn" | "aux" | "nul")
            || matches!(
                stem,
                "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9"
            )
            || matches!(
                stem,
                "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9"
            )
        {
            return false;
        }
        part.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    })
}

fn read_exact<R: Read>(source: &mut R, bytes: &mut [u8]) -> Result<(), BackupError> {
    source
        .read_exact(bytes)
        .map_err(|error| match error.kind() {
            io::ErrorKind::UnexpectedEof => BackupError::InvalidArchive,
            _ => BackupError::Io,
        })
}

fn require_end<R: Read>(source: &mut R) -> Result<(), BackupError> {
    let mut byte = [0u8; 1];
    match source.read(&mut byte) {
        Ok(0) => Ok(()),
        Ok(_) => Err(BackupError::InvalidArchive),
        Err(_) => Err(BackupError::Io),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &[u8] = b"synthetic backup password";

    fn manifest(size: u64) -> Manifest {
        Manifest {
            schema_version: 1,
            entries: vec![Entry {
                name: "records.sqlite".into(),
                size,
            }],
        }
    }

    fn encrypted(data: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        encrypt(
            &manifest(data.len() as u64),
            &mut Cursor::new(data),
            &mut output,
            PASSWORD,
            u64::MAX,
        )
        .unwrap();
        output
    }

    #[test]
    fn multichunk_archive_round_trips_with_fresh_randomness_and_authenticated_manifest() {
        let data: Vec<u8> = (0..(CHUNK_SIZE * 2 + 77))
            .map(|index| (index % 251) as u8)
            .collect();
        let mut expected = manifest(data.len() as u64 - 17);
        expected.entries.push(Entry {
            name: "evidence/receipt.txt".into(),
            size: 17,
        });
        let mut first = Vec::new();
        encrypt(
            &expected,
            &mut Cursor::new(&data),
            &mut first,
            PASSWORD,
            u64::MAX,
        )
        .unwrap();
        assert_eq!(encoded_size(&expected).unwrap(), first.len() as u64);
        let mut second = Vec::new();
        encrypt(
            &expected,
            &mut Cursor::new(&data),
            &mut second,
            PASSWORD,
            u64::MAX,
        )
        .unwrap();
        assert_ne!(first, second);
        assert!(
            !first
                .windows(b"records.sqlite".len())
                .any(|bytes| bytes == b"records.sqlite")
        );
        let mut verified = prepare_restore(
            &mut Cursor::new(first),
            PASSWORD,
            &std::env::temp_dir(),
            u64::MAX,
        )
        .unwrap();
        assert_eq!(verified.manifest(), &expected);
        let mut restored = Vec::new();
        assert_eq!(
            verified.copy_payload(&mut restored).unwrap(),
            data.len() as u64
        );
        assert_eq!(restored, data);
    }

    #[test]
    fn wrong_password_and_changed_missing_repeated_reordered_or_truncated_chunks_are_rejected() {
        let original = encrypted(&vec![0x5a; CHUNK_SIZE * 2 + 3]);
        assert!(matches!(
            prepare_restore(
                &mut Cursor::new(&original),
                b"wrong password",
                &std::env::temp_dir(),
                u64::MAX
            ),
            Err(BackupError::AuthenticationFailed)
        ));
        let first_end = HEADER_SIZE + FRAME_SIZE + CHUNK_SIZE + TAG_SIZE;
        let second_end = first_end + FRAME_SIZE + CHUNK_SIZE + TAG_SIZE;
        let mut changed_header = original.clone();
        changed_header[8] = 2;
        let mut changed_data = original.clone();
        changed_data[HEADER_SIZE + FRAME_SIZE + 3] ^= 1;
        let mut reordered = original[..HEADER_SIZE].to_vec();
        reordered.extend_from_slice(&original[first_end..second_end]);
        reordered.extend_from_slice(&original[HEADER_SIZE..first_end]);
        reordered.extend_from_slice(&original[second_end..]);
        let mut repeated = original[..first_end].to_vec();
        repeated.extend_from_slice(&original[HEADER_SIZE..first_end]);
        repeated.extend_from_slice(&original[second_end..]);
        let mut missing = original[..first_end].to_vec();
        missing.extend_from_slice(&original[second_end..]);
        let mut appended = original.clone();
        appended.push(0);
        let variants = [
            changed_header,
            changed_data,
            reordered,
            repeated,
            missing,
            original[..original.len() - 1].to_vec(),
            original[..second_end].to_vec(),
            appended,
        ];
        for bytes in variants {
            assert!(
                prepare_restore(
                    &mut Cursor::new(bytes),
                    PASSWORD,
                    &std::env::temp_dir(),
                    u64::MAX
                )
                .is_err()
            );
        }
    }

    #[test]
    fn oversized_archives_and_insufficient_space_fail_before_source_data_or_output() {
        struct NeverRead;
        impl Read for NeverRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("The codec must reject this manifest before it reads source data.");
            }
        }
        let mut output = Vec::new();
        assert_eq!(
            encrypt(
                &manifest(MAX_RESTORED_SIZE + 1),
                &mut NeverRead,
                &mut output,
                PASSWORD,
                u64::MAX
            ),
            Err(BackupError::LimitExceeded)
        );
        assert_eq!(
            encrypt(
                &manifest(MAX_RESTORED_SIZE),
                &mut NeverRead,
                &mut output,
                PASSWORD,
                0
            ),
            Err(BackupError::InsufficientSpace)
        );
        let mut many = Manifest {
            schema_version: 1,
            entries: (0..MAX_ENTRIES)
                .map(|index| Entry {
                    name: format!("data/{index}"),
                    size: 0,
                })
                .collect(),
        };
        assert_eq!(
            encrypt(&many, &mut NeverRead, &mut output, PASSWORD, 0),
            Err(BackupError::InsufficientSpace)
        );
        many.entries.push(Entry {
            name: "data/extra".into(),
            size: 0,
        });
        assert_eq!(
            encrypt(&many, &mut NeverRead, &mut output, PASSWORD, u64::MAX),
            Err(BackupError::LimitExceeded)
        );
        assert!(output.is_empty());

        let mut header = [0u8; HEADER_SIZE];
        header[..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        header[12..16].copy_from_slice(&1u32.to_le_bytes());
        header[16..24].copy_from_slice(&(MAX_RESTORED_SIZE + 1).to_le_bytes());
        header[24..28].copy_from_slice(&24u32.to_le_bytes());
        assert!(matches!(
            prepare_restore(
                &mut Cursor::new(header),
                PASSWORD,
                &std::env::temp_dir(),
                u64::MAX
            ),
            Err(BackupError::LimitExceeded)
        ));
        header[16..24].copy_from_slice(&0u64.to_le_bytes());
        assert!(matches!(
            prepare_restore(&mut Cursor::new(header), PASSWORD, &std::env::temp_dir(), 0),
            Err(BackupError::InsufficientSpace)
        ));
    }

    #[test]
    fn authenticated_unsafe_or_inconsistent_manifests_cannot_be_accepted() {
        let mut unsafe_manifest = manifest(0);
        unsafe_manifest.entries[0].name = "../records.sqlite".into();
        assert_eq!(
            encoded_size(&unsafe_manifest),
            Err(BackupError::InvalidManifest)
        );
        let original = encrypted(b"safe synthetic data");
        let header = Header::parse(original[..HEADER_SIZE].try_into().unwrap()).unwrap();
        let key = derive_key(PASSWORD, &header.bytes[28..44]).unwrap();
        let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).unwrap();
        let frame_bytes: [u8; FRAME_SIZE] = original[HEADER_SIZE..HEADER_SIZE + FRAME_SIZE]
            .try_into()
            .unwrap();
        let aad = additional_data(&header.bytes, &frame_bytes);
        let plaintext = cipher
            .decrypt(
                &nonce(&header, 0),
                Payload {
                    msg: &original[HEADER_SIZE + FRAME_SIZE..],
                    aad: &aad,
                },
            )
            .unwrap();
        for invalid in [b"../records.sql".as_slice(), b"con.txt.......".as_slice()] {
            let mut changed = Zeroizing::new(plaintext.clone());
            changed[10..24].copy_from_slice(invalid);
            let encrypted = cipher
                .encrypt(
                    &nonce(&header, 0),
                    Payload {
                        msg: &changed,
                        aad: &aad,
                    },
                )
                .unwrap();
            let mut archive = original[..HEADER_SIZE + FRAME_SIZE].to_vec();
            archive.extend_from_slice(&encrypted);
            assert!(matches!(
                prepare_restore(
                    &mut Cursor::new(archive),
                    PASSWORD,
                    &std::env::temp_dir(),
                    u64::MAX
                ),
                Err(BackupError::InvalidManifest)
            ));
        }
        let mut changed = Zeroizing::new(plaintext);
        changed[2..10].copy_from_slice(&0u64.to_le_bytes());
        let encrypted = cipher
            .encrypt(
                &nonce(&header, 0),
                Payload {
                    msg: &changed,
                    aad: &aad,
                },
            )
            .unwrap();
        let mut archive = original[..HEADER_SIZE + FRAME_SIZE].to_vec();
        archive.extend_from_slice(&encrypted);
        assert!(matches!(
            prepare_restore(
                &mut Cursor::new(archive),
                PASSWORD,
                &std::env::temp_dir(),
                u64::MAX
            ),
            Err(BackupError::InvalidManifest)
        ));
    }
}
