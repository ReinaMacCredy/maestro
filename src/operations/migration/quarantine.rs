use std::collections::BTreeSet;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::migration::runtime::{
    MigrationDigestV1, MigrationIdentityErrorV1, SealedQuarantineManifestV1,
};
use crate::foundation::core::secure_fs::{SecureDirectoryEntryKind, SecureFsError, SecureRoot};

const CHUNKS_DIRECTORY: &str = "chunks";
const MANIFEST_FILE: &str = "quarantine.v1.cbor";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineMaterializationReceiptV1 {
    manifest_id: MigrationDigestV1,
    quarantine_root: PathBuf,
    chunk_count: usize,
    byte_count: u64,
}

impl QuarantineMaterializationReceiptV1 {
    pub const fn manifest_id(&self) -> MigrationDigestV1 {
        self.manifest_id
    }

    pub fn quarantine_root(&self) -> &Path {
        &self.quarantine_root
    }

    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
}

pub fn materialize_sealed_quarantine(
    manifest: &SealedQuarantineManifestV1,
    destination: impl AsRef<Path>,
) -> Result<QuarantineMaterializationReceiptV1, QuarantineMaterializationErrorV1> {
    let destination = destination.as_ref();
    if !destination.is_absolute()
        || destination.as_os_str().as_bytes() != manifest.quarantine_root().as_bytes()
    {
        return Err(QuarantineMaterializationErrorV1::DestinationMismatch);
    }
    let root = SecureRoot::open_or_create(destination)?;
    for entry in root.read_dir_entries()? {
        let allowed = (entry.name() == Path::new(CHUNKS_DIRECTORY)
            && entry.kind() == SecureDirectoryEntryKind::Directory)
            || (entry.name() == Path::new(MANIFEST_FILE)
                && entry.kind() == SecureDirectoryEntryKind::RegularFile);
        if !allowed {
            return Err(QuarantineMaterializationErrorV1::UnexpectedEntry);
        }
    }
    root.create_dir_all(CHUNKS_DIRECTORY)?;
    let expected_chunk_names = manifest
        .chunk_identity_set()
        .into_iter()
        .map(|digest| PathBuf::from(digest.render_hex()))
        .collect::<BTreeSet<_>>();
    let chunks_root = root.open_dir(CHUNKS_DIRECTORY)?;
    for entry in chunks_root.read_dir_entries()? {
        if entry.kind() != SecureDirectoryEntryKind::RegularFile
            || !expected_chunk_names.contains(entry.name())
        {
            return Err(QuarantineMaterializationErrorV1::UnexpectedEntry);
        }
    }
    let mut byte_count = 0_u64;
    for entry in manifest.entries() {
        for (chunk, digest) in entry.chunks().iter().zip(entry.chunk_digests()) {
            if MigrationDigestV1::digest_bytes(chunk)? != *digest {
                return Err(QuarantineMaterializationErrorV1::ChunkDigestMismatch);
            }
            let relative = PathBuf::from(CHUNKS_DIRECTORY).join(digest.render_hex());
            root.create_file_if_absent(&relative, chunk)?;
            root.read_exact(&relative, chunk)?;
            byte_count = byte_count
                .checked_add(chunk.len() as u64)
                .ok_or(QuarantineMaterializationErrorV1::ByteLengthOverflow)?;
        }
    }
    root.create_file_if_absent(MANIFEST_FILE, manifest.canonical_bytes())?;
    root.read_exact(MANIFEST_FILE, manifest.canonical_bytes())?;
    root.verify_path_binding()?;
    let observed_root_entries = root
        .read_dir_entries()?
        .into_iter()
        .map(|entry| {
            let kind = match entry.kind() {
                SecureDirectoryEntryKind::RegularFile => 1_u8,
                SecureDirectoryEntryKind::Directory => 2_u8,
            };
            (entry.name().to_path_buf(), kind)
        })
        .collect::<BTreeSet<_>>();
    let expected_root_entries = BTreeSet::from([
        (PathBuf::from(CHUNKS_DIRECTORY), 2_u8),
        (PathBuf::from(MANIFEST_FILE), 1_u8),
    ]);
    if observed_root_entries != expected_root_entries {
        return Err(QuarantineMaterializationErrorV1::UnexpectedEntry);
    }
    let observed_chunk_names = chunks_root
        .read_dir_entries()?
        .into_iter()
        .map(|entry| entry.name().to_path_buf())
        .collect::<BTreeSet<_>>();
    if observed_chunk_names != expected_chunk_names {
        return Err(QuarantineMaterializationErrorV1::UnexpectedEntry);
    }
    for entry in manifest.entries() {
        let mut reconstructed = Vec::new();
        for digest in entry.chunk_digests() {
            let relative = PathBuf::from(CHUNKS_DIRECTORY).join(digest.render_hex());
            reconstructed.extend_from_slice(&root.read_immutable(relative)?);
        }
        if reconstructed.len() as u64 != entry.source_byte_length()
            || MigrationDigestV1::digest_bytes(&reconstructed)? != entry.source_sha256()
        {
            return Err(QuarantineMaterializationErrorV1::ReplayMismatch);
        }
    }
    Ok(QuarantineMaterializationReceiptV1 {
        manifest_id: manifest.id(),
        quarantine_root: destination.to_path_buf(),
        chunk_count: manifest.chunk_identity_set().len(),
        byte_count,
    })
}

#[derive(Debug, Error)]
pub enum QuarantineMaterializationErrorV1 {
    #[error("quarantine destination does not equal the sealed manifest locator")]
    DestinationMismatch,
    #[error("quarantine chunk bytes do not match their bound digest")]
    ChunkDigestMismatch,
    #[error("sealed quarantine replay does not reconstruct exact source bytes")]
    ReplayMismatch,
    #[error("quarantine byte length overflowed")]
    ByteLengthOverflow,
    #[error("sealed quarantine namespace contains a missing, extra, or unsafe entry")]
    UnexpectedEntry,
    #[error(transparent)]
    SecureFilesystem(#[from] SecureFsError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
}
