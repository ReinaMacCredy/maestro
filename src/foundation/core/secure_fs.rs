use std::io;
use std::path::PathBuf;

use thiserror::Error;

const MAX_PATH_BYTES: usize = 4_096;
const MAX_COMPONENTS: usize = 64;
const MAX_COMPONENT_BYTES: usize = 255;
const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;

pub type SecureFsResult<T> = Result<T, SecureFsError>;

#[derive(Debug, Error)]
pub enum SecureFsError {
    #[error("secure filesystem operations are unsupported on {platform}")]
    UnsupportedPlatform { platform: &'static str },
    #[error("invalid secure filesystem path {path}: {reason}")]
    InvalidPath { path: PathBuf, reason: &'static str },
    #[error("failed to {operation} {path}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unsafe filesystem object at {path}: {reason}")]
    UnsafeObject { path: PathBuf, reason: &'static str },
    #[error("{path} changed while it was being read")]
    ChangedDuringRead { path: PathBuf },
    #[error("{path} does not match the expected immutable bytes")]
    ContentMismatch { path: PathBuf },
}

impl SecureFsError {
    fn io_kind(&self, kind: io::ErrorKind) -> bool {
        matches!(self, Self::Io { source, .. } if source.kind() == kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateIfAbsent {
    Created,
    AlreadyExists,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RegularFileBinding {
    identity: platform::RegularFileIdentity,
}

impl std::fmt::Debug for RegularFileBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RegularFileBinding(..)")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform {
    use std::ffi::{CStr, CString, OsStr};
    use std::fs::{File, Metadata};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Component, Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sha2::{Digest, Sha256};

    use super::{
        CreateIfAbsent, MAX_COMPONENT_BYTES, MAX_COMPONENTS, MAX_PATH_BYTES, RegularFileBinding,
        SecureFsError, SecureFsResult,
    };

    #[cfg(target_os = "linux")]
    type ModeT = c_uint;
    #[cfg(target_os = "macos")]
    type ModeT = u16;

    const DIRECTORY_MODE: ModeT = 0o700;
    const FILE_MODE: c_int = 0o600;
    const WRITABLE_BY_OTHERS: u32 = 0o022;
    const TEMP_ATTEMPTS: usize = 32;
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[cfg(target_os = "linux")]
    mod flags {
        use std::os::raw::c_int;
        pub const O_RDONLY: c_int = 0;
        pub const O_WRONLY: c_int = 1;
        pub const O_CREAT: c_int = 0o100;
        pub const O_EXCL: c_int = 0o200;
        pub const O_DIRECTORY: c_int = 0o200000;
        pub const O_NOFOLLOW: c_int = 0o400000;
        pub const O_CLOEXEC: c_int = 0o2000000;
        pub const AT_REMOVEDIR: c_int = 0x200;
    }

    #[cfg(target_os = "macos")]
    mod flags {
        use std::os::raw::c_int;
        pub const O_RDONLY: c_int = 0;
        pub const O_WRONLY: c_int = 1;
        pub const O_NOFOLLOW: c_int = 0x100;
        pub const O_CREAT: c_int = 0x200;
        pub const O_EXCL: c_int = 0x800;
        pub const O_DIRECTORY: c_int = 0x10_0000;
        pub const O_CLOEXEC: c_int = 0x100_0000;
        pub const AT_REMOVEDIR: c_int = 0x80;
    }

    unsafe extern "C" {
        fn open(path: *const c_char, flags: c_int, ...) -> c_int;
        fn openat(directory: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
        fn mkdirat(directory: c_int, path: *const c_char, mode: ModeT) -> c_int;
        #[cfg(target_os = "linux")]
        fn renameat2(
            old_directory: c_int,
            old_path: *const c_char,
            new_directory: c_int,
            new_path: *const c_char,
            flags: c_uint,
        ) -> c_int;
        #[cfg(target_os = "macos")]
        fn renameatx_np(
            old_directory: c_int,
            old_path: *const c_char,
            new_directory: c_int,
            new_path: *const c_char,
            flags: c_uint,
        ) -> c_int;
        fn unlinkat(directory: c_int, path: *const c_char, flags: c_int) -> c_int;
        fn geteuid() -> c_uint;
    }

    #[derive(Debug)]
    pub struct SecureRoot {
        directory: File,
        path: PathBuf,
    }

    impl SecureRoot {
        pub fn open(path: impl AsRef<Path>) -> SecureFsResult<Self> {
            open_root(path.as_ref(), false)
        }

        pub fn open_or_create(path: impl AsRef<Path>) -> SecureFsResult<Self> {
            open_root(path.as_ref(), true)
        }

        pub fn path(&self) -> &Path {
            &self.path
        }

        pub fn verify_path_binding(&self) -> SecureFsResult<()> {
            let reopened = open_root(&self.path, false)?;
            if ObjectIdentity::from(&metadata(&self.directory, &self.path)?)
                != ObjectIdentity::from(&metadata(&reopened.directory, &self.path)?)
            {
                return Err(SecureFsError::ChangedDuringRead {
                    path: self.path.clone(),
                });
            }
            Ok(())
        }

        pub fn open_dir(&self, relative: impl AsRef<Path>) -> SecureFsResult<Self> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let mut directory = self.clone_directory()?;
            for component in &relative.components {
                directory = open_directory_at(
                    directory.as_raw_fd(),
                    component,
                    &self.path.join(&relative.path),
                )?;
            }
            let path = self.path.join(relative.path);
            validate_directory(&directory, &path)?;
            Ok(Self { directory, path })
        }

        pub fn create_dir_all(&self, relative: impl AsRef<Path>) -> SecureFsResult<()> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let mut directory = self.clone_directory()?;
            let mut traversed = PathBuf::new();
            for component in &relative.components {
                traversed.push(OsStr::from_bytes(component.to_bytes()));
                directory =
                    ensure_directory_at(&directory, component, &self.path.join(&traversed))?;
            }
            Ok(())
        }

        pub fn create_file_if_absent(
            &self,
            relative: impl AsRef<Path>,
            contents: &[u8],
        ) -> SecureFsResult<CreateIfAbsent> {
            if contents.len() > super::MAX_FILE_BYTES {
                return Err(SecureFsError::UnsafeObject {
                    path: self.path.join(relative.as_ref()),
                    reason: "file exceeds the secure byte limit",
                });
            }
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let temp = create_temp_file(&parent, contents, &path)?;
            let publish = rename_no_replace(parent.as_raw_fd(), &temp.name, leaf, &path);

            match publish {
                Ok(()) => {
                    verify_published_file(&parent, leaf, &temp.file, contents, &path)?;
                    sync_directory(&parent, &path)?;
                    Ok(CreateIfAbsent::Created)
                }
                Err(error) if error.io_kind(io::ErrorKind::AlreadyExists) => {
                    cleanup_temp(&parent, &temp.name, &path)?;
                    let existing = open_regular_file_at(&parent, leaf, &path)?;
                    validate_regular_file(&existing, &path)?;
                    Ok(CreateIfAbsent::AlreadyExists)
                }
                Err(error) => {
                    let _ = cleanup_temp(&parent, &temp.name, &path);
                    Err(error)
                }
            }
        }

        pub fn rename_file_no_replace(
            &self,
            source: impl AsRef<Path>,
            destination: impl AsRef<Path>,
        ) -> SecureFsResult<CreateIfAbsent> {
            let source = BoundedPath::new(source.as_ref())?;
            let destination = BoundedPath::new(destination.as_ref())?;
            let (source_leaf, source_parents) = source
                .components
                .split_last()
                .expect("invariant: bounded source path has a component");
            let (destination_leaf, destination_parents) = destination
                .components
                .split_last()
                .expect("invariant: bounded destination path has a component");
            if source_parents != destination_parents {
                return Err(invalid_path(
                    &destination.path,
                    "no-replace rename must stay within one secure directory",
                ));
            }
            let (parent, _, source_path) = self.open_parent(&source)?;
            let source_file = open_regular_file_at(&parent, source_leaf, &source_path)?;
            validate_regular_file(&source_file, &source_path)?;
            let destination_path = self.path.join(&destination.path);
            match rename_no_replace(
                parent.as_raw_fd(),
                source_leaf,
                destination_leaf,
                &destination_path,
            ) {
                Ok(()) => {
                    verify_published_identity(
                        &parent,
                        destination_leaf,
                        &source_file,
                        &destination_path,
                    )?;
                    sync_directory(&parent, &destination_path)?;
                    Ok(CreateIfAbsent::Created)
                }
                Err(error) if error.io_kind(io::ErrorKind::AlreadyExists) => {
                    Ok(CreateIfAbsent::AlreadyExists)
                }
                Err(error) => Err(error),
            }
        }

        pub fn read_immutable(&self, relative: impl AsRef<Path>) -> SecureFsResult<Vec<u8>> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let mut file = open_regular_file_at(&parent, leaf, &path)?;
            validate_regular_file(&file, &path)?;
            read_stable(&mut file, &path)
        }

        pub fn validate_regular_file(&self, relative: impl AsRef<Path>) -> SecureFsResult<()> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let file = open_regular_file_at(&parent, leaf, &path)?;
            validate_regular_file(&file, &path)
        }

        pub(crate) fn bind_regular_file(
            &self,
            relative: impl AsRef<Path>,
        ) -> SecureFsResult<RegularFileBinding> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let file = open_regular_file_at(&parent, leaf, &path)?;
            validate_regular_file(&file, &path)?;
            Ok(RegularFileBinding {
                identity: ObjectIdentity::from(&metadata(&file, &path)?),
            })
        }

        pub(crate) fn bind_optional_regular_file(
            &self,
            relative: impl AsRef<Path>,
        ) -> SecureFsResult<Option<RegularFileBinding>> {
            match self.bind_regular_file(relative) {
                Ok(binding) => Ok(Some(binding)),
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => Ok(None),
                Err(error) => Err(error),
            }
        }

        pub(crate) fn verify_regular_file_binding(
            &self,
            relative: impl AsRef<Path>,
            expected: &RegularFileBinding,
        ) -> SecureFsResult<()> {
            let relative = relative.as_ref();
            if self.bind_regular_file(relative)? != *expected {
                return Err(SecureFsError::ChangedDuringRead {
                    path: self.path.join(relative),
                });
            }
            Ok(())
        }

        pub(crate) fn verify_optional_regular_file_binding(
            &self,
            relative: impl AsRef<Path>,
            expected: Option<&RegularFileBinding>,
        ) -> SecureFsResult<()> {
            let relative = relative.as_ref();
            let observed = self.bind_optional_regular_file(relative)?;
            if observed.as_ref() != expected {
                return Err(SecureFsError::ChangedDuringRead {
                    path: self.path.join(relative),
                });
            }
            Ok(())
        }

        pub fn read_exact(
            &self,
            relative: impl AsRef<Path>,
            expected: &[u8],
        ) -> SecureFsResult<Vec<u8>> {
            let relative = relative.as_ref();
            let bytes = self.read_immutable(relative)?;
            if bytes != expected {
                return Err(SecureFsError::ContentMismatch {
                    path: self.path.join(relative),
                });
            }
            Ok(bytes)
        }

        pub fn remove_file_if_matches(
            &self,
            relative: impl AsRef<Path>,
            expected: &[u8],
        ) -> SecureFsResult<bool> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let quarantine = removal_quarantine_name(expected);
            let mut file = match open_regular_file_at(&parent, leaf, &path) {
                Ok(file) => file,
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                    return finish_quarantined_removal(&parent, &quarantine, expected, &path);
                }
                Err(error) => return Err(error),
            };
            validate_regular_file(&file, &path)?;
            let opened_identity = ObjectIdentity::from(&metadata(&file, &path)?);
            if read_stable(&mut file, &path)? != expected {
                return Err(SecureFsError::ContentMismatch { path });
            }
            move_to_removal_quarantine(&parent, leaf, &quarantine, &path)?;
            let verification = (|| {
                let candidate = open_regular_file_at(&parent, &quarantine, &path)?;
                validate_regular_file(&candidate, &path)?;
                if ObjectIdentity::from(&metadata(&candidate, &path)?) != opened_identity {
                    return Err(SecureFsError::ChangedDuringRead { path: path.clone() });
                }
                Ok(())
            })();
            if let Err(error) = verification {
                restore_from_quarantine(&parent, &quarantine, leaf, &path)?;
                return Err(error);
            }
            unlink_at(
                parent.as_raw_fd(),
                &quarantine,
                0,
                &path,
                "remove quarantined file",
            )?;
            if metadata(&file, &path)?.nlink() != 0 {
                return Err(SecureFsError::UnsafeObject {
                    path,
                    reason: "removed file still has a hard-link alias",
                });
            }
            sync_directory(&parent, &path)?;
            Ok(true)
        }

        pub fn finish_file_removal_if_digest_matches(
            &self,
            relative: impl AsRef<Path>,
            expected_digest: &[u8; 32],
        ) -> SecureFsResult<bool> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, _, path) = self.open_parent(&relative)?;
            let quarantine = removal_quarantine_name_for_digest(expected_digest);
            finish_quarantined_removal_by_digest(&parent, &quarantine, expected_digest, &path)
        }

        pub fn remove_empty_dir(&self, relative: impl AsRef<Path>) -> SecureFsResult<bool> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let directory = match open_directory_at(parent.as_raw_fd(), leaf, &path) {
                Ok(directory) => directory,
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => return Ok(false),
                Err(error) => return Err(error),
            };
            let opened_identity = ObjectIdentity::from(&metadata(&directory, &path)?);
            let quarantine = move_to_quarantine(&parent, leaf, &path)?;
            let verification = (|| {
                let candidate = open_directory_at(parent.as_raw_fd(), &quarantine, &path)?;
                if ObjectIdentity::from(&metadata(&candidate, &path)?) != opened_identity {
                    return Err(SecureFsError::ChangedDuringRead { path: path.clone() });
                }
                Ok(())
            })();
            if let Err(error) = verification {
                restore_from_quarantine(&parent, &quarantine, leaf, &path)?;
                return Err(error);
            }
            match unlink_at(
                parent.as_raw_fd(),
                &quarantine,
                flags::AT_REMOVEDIR,
                &path,
                "remove quarantined empty directory",
            ) {
                Ok(()) => {
                    sync_directory(&parent, &path)?;
                    Ok(true)
                }
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => Ok(false),
                Err(error) => Err(error),
            }
        }

        fn open_parent<'a>(
            &self,
            relative: &'a BoundedPath,
        ) -> SecureFsResult<(File, &'a CStr, PathBuf)> {
            let (leaf, parents) = relative
                .components
                .split_last()
                .expect("invariant: bounded path has a component");
            let mut directory = self.clone_directory()?;
            let mut traversed = PathBuf::new();
            for component in parents {
                traversed.push(OsStr::from_bytes(component.to_bytes()));
                directory = open_directory_at(
                    directory.as_raw_fd(),
                    component,
                    &self.path.join(&traversed),
                )?;
            }
            Ok((directory, leaf, self.path.join(&relative.path)))
        }

        fn clone_directory(&self) -> SecureFsResult<File> {
            self.directory
                .try_clone()
                .map_err(|source| SecureFsError::Io {
                    operation: "duplicate directory descriptor",
                    path: self.path.clone(),
                    source,
                })
        }
    }

    struct BoundedPath {
        path: PathBuf,
        components: Vec<CString>,
    }

    impl BoundedPath {
        fn new(path: &Path) -> SecureFsResult<Self> {
            let bytes = path.as_os_str().as_bytes();
            if bytes.is_empty() {
                return Err(invalid_path(path, "path must not be empty"));
            }
            if bytes.len() > MAX_PATH_BYTES {
                return Err(invalid_path(path, "path exceeds the secure byte limit"));
            }
            let mut components = Vec::new();
            for component in path.components() {
                let Component::Normal(component) = component else {
                    return Err(invalid_path(
                        path,
                        "path must be relative and contain only normal components",
                    ));
                };
                if component.as_bytes().len() > MAX_COMPONENT_BYTES {
                    return Err(invalid_path(path, "path component exceeds the byte limit"));
                }
                components.push(c_string(component, path)?);
                if components.len() > MAX_COMPONENTS {
                    return Err(invalid_path(path, "path exceeds the component limit"));
                }
            }
            if components.is_empty() {
                return Err(invalid_path(path, "path must contain a name"));
            }
            Ok(Self {
                path: path.to_path_buf(),
                components,
            })
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(super) struct ObjectIdentity {
        device: u64,
        inode: u64,
    }

    pub(super) type RegularFileIdentity = ObjectIdentity;

    impl ObjectIdentity {
        fn from(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    struct FileIdentity {
        device: u64,
        inode: u64,
        len: u64,
        mtime: (i64, i64),
        ctime: (i64, i64),
        links: u64,
        owner: u32,
        mode: u32,
    }

    impl FileIdentity {
        fn from(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                len: metadata.len(),
                mtime: (metadata.mtime(), metadata.mtime_nsec()),
                ctime: (metadata.ctime(), metadata.ctime_nsec()),
                links: metadata.nlink(),
                owner: metadata.uid(),
                mode: metadata.mode(),
            }
        }
    }

    fn read_stable(file: &mut File, path: &Path) -> SecureFsResult<Vec<u8>> {
        let before = FileIdentity::from(&metadata(file, path)?);
        if before.len > super::MAX_FILE_BYTES as u64 {
            return Err(unsafe_object(path, "file exceeds the secure byte limit"));
        }
        let mut bytes = Vec::with_capacity(before.len as usize);
        {
            let mut bounded = file.take((super::MAX_FILE_BYTES as u64) + 1);
            bounded
                .read_to_end(&mut bytes)
                .map_err(|source| SecureFsError::Io {
                    operation: "read immutable file",
                    path: path.to_path_buf(),
                    source,
                })?;
        }
        if bytes.len() > super::MAX_FILE_BYTES {
            return Err(unsafe_object(path, "file exceeds the secure byte limit"));
        }
        let after = FileIdentity::from(&metadata(file, path)?);
        if before != after || before.len != bytes.len() as u64 {
            return Err(SecureFsError::ChangedDuringRead {
                path: path.to_path_buf(),
            });
        }
        Ok(bytes)
    }

    fn open_root(path: &Path, create: bool) -> SecureFsResult<SecureRoot> {
        let anchored = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|source| SecureFsError::Io {
                    operation: "resolve secure root against the current directory",
                    path: path.to_path_buf(),
                    source,
                })?
                .join(path)
        };
        validate_root_path(&anchored)?;
        let mut directory = open_directory(Path::new("/"))?;
        for component in anchored.components() {
            let Component::Normal(component) = component else {
                continue;
            };
            let name = c_string(component, &anchored)?;
            directory = if create {
                ensure_root_directory_at(&directory, &name, &anchored)?
            } else {
                open_directory_at_unchecked(directory.as_raw_fd(), &name, &anchored)?
            };
        }
        validate_directory(&directory, &anchored)?;
        Ok(SecureRoot {
            directory,
            path: anchored,
        })
    }

    fn validate_root_path(path: &Path) -> SecureFsResult<()> {
        let bytes = path.as_os_str().as_bytes();
        if bytes.is_empty() {
            return Err(invalid_path(path, "root path must not be empty"));
        }
        if bytes.len() > MAX_PATH_BYTES {
            return Err(invalid_path(path, "root path exceeds the byte limit"));
        }
        let mut count = 0;
        for component in path.components() {
            match component {
                Component::Prefix(_) | Component::ParentDir => {
                    return Err(invalid_path(
                        path,
                        "root path must not contain parent traversal",
                    ));
                }
                Component::Normal(component) => {
                    if component.as_bytes().len() > MAX_COMPONENT_BYTES {
                        return Err(invalid_path(path, "root component exceeds the byte limit"));
                    }
                    let _ = c_string(component, path)?;
                    count += 1;
                    if count > MAX_COMPONENTS {
                        return Err(invalid_path(path, "root path exceeds the component limit"));
                    }
                }
                Component::RootDir | Component::CurDir => {}
            }
        }
        Ok(())
    }

    fn open_directory(path: &Path) -> SecureFsResult<File> {
        let name = c_string(path.as_os_str(), path)?;
        let descriptor = unsafe {
            open(
                name.as_ptr(),
                flags::O_RDONLY | flags::O_DIRECTORY | flags::O_NOFOLLOW | flags::O_CLOEXEC,
            )
        };
        descriptor_to_file(descriptor, "open directory", path)
    }

    fn open_directory_at(directory: RawFd, name: &CStr, path: &Path) -> SecureFsResult<File> {
        let file = open_directory_at_unchecked(directory, name, path)?;
        validate_directory(&file, path)?;
        Ok(file)
    }

    fn open_directory_at_unchecked(
        directory: RawFd,
        name: &CStr,
        path: &Path,
    ) -> SecureFsResult<File> {
        let descriptor = unsafe {
            openat(
                directory,
                name.as_ptr(),
                flags::O_RDONLY | flags::O_DIRECTORY | flags::O_NOFOLLOW | flags::O_CLOEXEC,
            )
        };
        descriptor_to_file(descriptor, "open directory without following links", path)
    }

    fn ensure_root_directory_at(parent: &File, name: &CStr, path: &Path) -> SecureFsResult<File> {
        match open_directory_at_unchecked(parent.as_raw_fd(), name, path) {
            Ok(directory) => return Ok(directory),
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {}
            Err(error) => return Err(error),
        }
        let result = unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), DIRECTORY_MODE) };
        if result == -1 {
            let source = io::Error::last_os_error();
            if source.kind() != io::ErrorKind::AlreadyExists {
                return Err(SecureFsError::Io {
                    operation: "create root directory",
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        sync_directory(parent, path)?;
        open_directory_at_unchecked(parent.as_raw_fd(), name, path)
    }

    fn ensure_directory_at(parent: &File, name: &CStr, path: &Path) -> SecureFsResult<File> {
        match open_directory_at(parent.as_raw_fd(), name, path) {
            Ok(directory) => return Ok(directory),
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {}
            Err(error) => return Err(error),
        }
        let result = unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), DIRECTORY_MODE) };
        if result == -1 {
            let source = io::Error::last_os_error();
            if source.kind() != io::ErrorKind::AlreadyExists {
                return Err(SecureFsError::Io {
                    operation: "create directory",
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        sync_directory(parent, path)?;
        open_directory_at(parent.as_raw_fd(), name, path)
    }

    fn open_regular_file_at(parent: &File, name: &CStr, path: &Path) -> SecureFsResult<File> {
        let descriptor = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                flags::O_RDONLY | flags::O_NOFOLLOW | flags::O_CLOEXEC,
            )
        };
        descriptor_to_file(descriptor, "open file without following links", path)
    }

    struct PendingFile {
        name: CString,
        file: File,
    }

    fn create_temp_file(
        parent: &File,
        contents: &[u8],
        path: &Path,
    ) -> SecureFsResult<PendingFile> {
        let mut last_collision = None;
        for _ in 0..TEMP_ATTEMPTS {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let name = CString::new(format!(".maestro-secure-{}.{}.tmp", process::id(), counter))
                .expect("invariant: generated temporary name has no nul byte");
            let descriptor = unsafe {
                openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    flags::O_WRONLY
                        | flags::O_CREAT
                        | flags::O_EXCL
                        | flags::O_NOFOLLOW
                        | flags::O_CLOEXEC,
                    FILE_MODE,
                )
            };
            if descriptor == -1 {
                let source = io::Error::last_os_error();
                if source.kind() == io::ErrorKind::AlreadyExists {
                    last_collision = Some(source);
                    continue;
                }
                return Err(SecureFsError::Io {
                    operation: "create temporary file without following links",
                    path: path.to_path_buf(),
                    source,
                });
            }
            let mut file = unsafe { File::from_raw_fd(descriptor) };
            if let Err(source) = file.write_all(contents).and_then(|()| file.sync_all()) {
                let _ = unlink_at(
                    parent.as_raw_fd(),
                    &name,
                    0,
                    path,
                    "remove incomplete temporary file",
                );
                let _ = sync_directory(parent, path);
                return Err(SecureFsError::Io {
                    operation: "write and sync temporary file",
                    path: path.to_path_buf(),
                    source,
                });
            }
            return Ok(PendingFile { name, file });
        }
        Err(SecureFsError::Io {
            operation: "allocate a unique temporary file",
            path: path.to_path_buf(),
            source: last_collision.unwrap_or_else(|| io::ErrorKind::AlreadyExists.into()),
        })
    }

    fn verify_published_file(
        parent: &File,
        leaf: &CStr,
        source: &File,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        let mut published = verify_published_identity(parent, leaf, source, path)?;
        let bytes = read_stable(&mut published, path)?;
        if bytes != expected {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn verify_published_identity(
        parent: &File,
        leaf: &CStr,
        source: &File,
        path: &Path,
    ) -> SecureFsResult<File> {
        let published = open_regular_file_at(parent, leaf, path)?;
        validate_regular_file(&published, path)?;
        if ObjectIdentity::from(&metadata(source, path)?)
            != ObjectIdentity::from(&metadata(&published, path)?)
        {
            return Err(unsafe_object(
                path,
                "published file identity changed during no-replace rename",
            ));
        }
        Ok(published)
    }

    fn rename_no_replace(
        directory: RawFd,
        old_name: &CStr,
        new_name: &CStr,
        path: &Path,
    ) -> SecureFsResult<()> {
        #[cfg(target_os = "linux")]
        let result = unsafe {
            const RENAME_NOREPLACE: c_uint = 1;
            renameat2(
                directory,
                old_name.as_ptr(),
                directory,
                new_name.as_ptr(),
                RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let result = unsafe {
            const RENAME_EXCL: c_uint = 0x0000_0004;
            renameatx_np(
                directory,
                old_name.as_ptr(),
                directory,
                new_name.as_ptr(),
                RENAME_EXCL,
            )
        };
        result_to_unit(
            result,
            "rename file without replacing an existing leaf",
            path,
        )
    }

    fn removal_quarantine_name(expected: &[u8]) -> CString {
        let digest: [u8; 32] = Sha256::digest(expected).into();
        removal_quarantine_name_for_digest(&digest)
    }

    fn removal_quarantine_name_for_digest(digest: &[u8; 32]) -> CString {
        let mut name = String::from(".maestro-remove-");
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut name, "{byte:02x}")
                .expect("invariant: writing a digest into a String cannot fail");
        }
        name.push_str(".pending");
        CString::new(name).expect("invariant: generated quarantine name has no nul byte")
    }

    fn move_to_removal_quarantine(
        parent: &File,
        leaf: &CStr,
        quarantine: &CStr,
        path: &Path,
    ) -> SecureFsResult<()> {
        rename_no_replace(parent.as_raw_fd(), leaf, quarantine, path)?;
        sync_directory(parent, path)
    }

    fn finish_quarantined_removal(
        parent: &File,
        quarantine: &CStr,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, quarantine, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => return Ok(false),
            Err(error) => return Err(error),
        };
        validate_regular_file(&candidate, path)?;
        if read_stable(&mut candidate, path)? != expected {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        unlink_at(
            parent.as_raw_fd(),
            quarantine,
            0,
            path,
            "remove recovered quarantined file",
        )?;
        if metadata(&candidate, path)?.nlink() != 0 {
            return Err(SecureFsError::UnsafeObject {
                path: path.to_path_buf(),
                reason: "removed file still has a hard-link alias",
            });
        }
        sync_directory(parent, path)?;
        Ok(true)
    }

    fn finish_quarantined_removal_by_digest(
        parent: &File,
        quarantine: &CStr,
        expected_digest: &[u8; 32],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, quarantine, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => return Ok(false),
            Err(error) => return Err(error),
        };
        validate_regular_file(&candidate, path)?;
        let bytes = read_stable(&mut candidate, path)?;
        let observed: [u8; 32] = Sha256::digest(&bytes).into();
        if &observed != expected_digest {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        unlink_at(
            parent.as_raw_fd(),
            quarantine,
            0,
            path,
            "remove recovered quarantined file",
        )?;
        if metadata(&candidate, path)?.nlink() != 0 {
            return Err(SecureFsError::UnsafeObject {
                path: path.to_path_buf(),
                reason: "removed file still has a hard-link alias",
            });
        }
        sync_directory(parent, path)?;
        Ok(true)
    }

    fn move_to_quarantine(parent: &File, leaf: &CStr, path: &Path) -> SecureFsResult<CString> {
        let mut last_collision = None;
        for _ in 0..TEMP_ATTEMPTS {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let quarantine = CString::new(format!(
                ".maestro-secure-{}.{}.remove",
                process::id(),
                counter
            ))
            .expect("invariant: generated quarantine name has no nul byte");
            match rename_no_replace(parent.as_raw_fd(), leaf, &quarantine, path) {
                Ok(()) => {
                    sync_directory(parent, path)?;
                    return Ok(quarantine);
                }
                Err(error) if error.io_kind(io::ErrorKind::AlreadyExists) => {
                    last_collision = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_collision.unwrap_or_else(|| SecureFsError::Io {
            operation: "allocate a unique removal quarantine",
            path: path.to_path_buf(),
            source: io::ErrorKind::AlreadyExists.into(),
        }))
    }

    fn restore_from_quarantine(
        parent: &File,
        quarantine: &CStr,
        leaf: &CStr,
        path: &Path,
    ) -> SecureFsResult<()> {
        rename_no_replace(parent.as_raw_fd(), quarantine, leaf, path)?;
        sync_directory(parent, path)
    }

    fn cleanup_temp(parent: &File, name: &CStr, path: &Path) -> SecureFsResult<()> {
        let removal = unlink_at(parent.as_raw_fd(), name, 0, path, "remove temporary file");
        let sync = sync_directory(parent, path);
        removal?;
        sync
    }

    fn unlink_at(
        directory: RawFd,
        name: &CStr,
        flags: c_int,
        path: &Path,
        operation: &'static str,
    ) -> SecureFsResult<()> {
        let result = unsafe { unlinkat(directory, name.as_ptr(), flags) };
        result_to_unit(result, operation, path)
    }

    fn sync_directory(directory: &File, path: &Path) -> SecureFsResult<()> {
        directory.sync_all().map_err(|source| SecureFsError::Io {
            operation: "sync parent directory",
            path: path.to_path_buf(),
            source,
        })
    }

    fn validate_directory(file: &File, path: &Path) -> SecureFsResult<()> {
        let metadata = metadata(file, path)?;
        if !metadata.is_dir() {
            return Err(unsafe_object(path, "expected a directory"));
        }
        validate_owner_and_mode(&metadata, path)
    }

    fn validate_regular_file(file: &File, path: &Path) -> SecureFsResult<()> {
        let metadata = metadata(file, path)?;
        if !metadata.is_file() {
            return Err(unsafe_object(path, "expected a regular file"));
        }
        if metadata.nlink() != 1 {
            return Err(unsafe_object(path, "immutable file has a hard-link alias"));
        }
        validate_owner_and_mode(&metadata, path)
    }

    fn validate_owner_and_mode(metadata: &Metadata, path: &Path) -> SecureFsResult<()> {
        if metadata.uid() != unsafe { geteuid() } {
            return Err(unsafe_object(
                path,
                "object is not owned by the effective user",
            ));
        }
        if metadata.mode() & WRITABLE_BY_OTHERS != 0 {
            return Err(unsafe_object(
                path,
                "object is writable by group or other users",
            ));
        }
        Ok(())
    }

    fn metadata(file: &File, path: &Path) -> SecureFsResult<Metadata> {
        file.metadata().map_err(|source| SecureFsError::Io {
            operation: "inspect opened filesystem object",
            path: path.to_path_buf(),
            source,
        })
    }

    fn descriptor_to_file(
        descriptor: c_int,
        operation: &'static str,
        path: &Path,
    ) -> SecureFsResult<File> {
        if descriptor == -1 {
            return Err(SecureFsError::Io {
                operation,
                path: path.to_path_buf(),
                source: io::Error::last_os_error(),
            });
        }
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    fn result_to_unit(result: c_int, operation: &'static str, path: &Path) -> SecureFsResult<()> {
        if result == -1 {
            return Err(SecureFsError::Io {
                operation,
                path: path.to_path_buf(),
                source: io::Error::last_os_error(),
            });
        }
        Ok(())
    }

    fn c_string(value: &OsStr, path: &Path) -> SecureFsResult<CString> {
        CString::new(value.as_bytes()).map_err(|_| invalid_path(path, "path contains a nul byte"))
    }

    fn invalid_path(path: &Path, reason: &'static str) -> SecureFsError {
        SecureFsError::InvalidPath {
            path: path.to_path_buf(),
            reason,
        }
    }

    fn unsafe_object(path: &Path, reason: &'static str) -> SecureFsError {
        SecureFsError::UnsafeObject {
            path: path.to_path_buf(),
            reason,
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod platform {
    use super::{CreateIfAbsent, RegularFileBinding, SecureFsError, SecureFsResult};
    use std::path::Path;

    #[derive(Debug)]
    pub struct SecureRoot;

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(super) struct RegularFileIdentity {
        _private: (),
    }

    impl SecureRoot {
        pub fn open(_path: impl AsRef<Path>) -> SecureFsResult<Self> {
            unsupported()
        }
        pub fn open_or_create(_path: impl AsRef<Path>) -> SecureFsResult<Self> {
            unsupported()
        }
        pub fn path(&self) -> &Path {
            Path::new("")
        }
        pub fn verify_path_binding(&self) -> SecureFsResult<()> {
            unsupported()
        }
        pub fn open_dir(&self, _path: impl AsRef<Path>) -> SecureFsResult<Self> {
            unsupported()
        }
        pub fn create_dir_all(&self, _path: impl AsRef<Path>) -> SecureFsResult<()> {
            unsupported()
        }
        pub fn create_file_if_absent(
            &self,
            _path: impl AsRef<Path>,
            _contents: &[u8],
        ) -> SecureFsResult<CreateIfAbsent> {
            unsupported()
        }
        pub fn rename_file_no_replace(
            &self,
            _source: impl AsRef<Path>,
            _destination: impl AsRef<Path>,
        ) -> SecureFsResult<CreateIfAbsent> {
            unsupported()
        }
        pub fn read_immutable(&self, _path: impl AsRef<Path>) -> SecureFsResult<Vec<u8>> {
            unsupported()
        }
        pub fn validate_regular_file(&self, _path: impl AsRef<Path>) -> SecureFsResult<()> {
            unsupported()
        }
        pub(crate) fn bind_regular_file(
            &self,
            _path: impl AsRef<Path>,
        ) -> SecureFsResult<RegularFileBinding> {
            unsupported()
        }
        pub(crate) fn bind_optional_regular_file(
            &self,
            _path: impl AsRef<Path>,
        ) -> SecureFsResult<Option<RegularFileBinding>> {
            unsupported()
        }
        pub(crate) fn verify_regular_file_binding(
            &self,
            _path: impl AsRef<Path>,
            _expected: &RegularFileBinding,
        ) -> SecureFsResult<()> {
            unsupported()
        }
        pub(crate) fn verify_optional_regular_file_binding(
            &self,
            _path: impl AsRef<Path>,
            _expected: Option<&RegularFileBinding>,
        ) -> SecureFsResult<()> {
            unsupported()
        }
        pub fn read_exact(
            &self,
            _path: impl AsRef<Path>,
            _expected: &[u8],
        ) -> SecureFsResult<Vec<u8>> {
            unsupported()
        }
        pub fn remove_file_if_matches(
            &self,
            _path: impl AsRef<Path>,
            _expected: &[u8],
        ) -> SecureFsResult<bool> {
            unsupported()
        }
        pub fn finish_file_removal_if_digest_matches(
            &self,
            _path: impl AsRef<Path>,
            _expected_digest: &[u8; 32],
        ) -> SecureFsResult<bool> {
            unsupported()
        }
        pub fn remove_empty_dir(&self, _path: impl AsRef<Path>) -> SecureFsResult<bool> {
            unsupported()
        }
    }

    fn unsupported<T>() -> SecureFsResult<T> {
        Err(SecureFsError::UnsupportedPlatform {
            platform: std::env::consts::OS,
        })
    }
}

pub use platform::SecureRoot;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{CreateIfAbsent, SecureFsError, SecureRoot};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let temp_root = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            let path = temp_root.join(format!("maestro-secure-fs-{}-{counter}", process::id()));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn publishes_without_replacing_existing_file() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        assert_eq!(
            root.create_file_if_absent("claim", b"first")
                .expect("create"),
            CreateIfAbsent::Created
        );
        assert_eq!(
            root.create_file_if_absent("claim", b"second")
                .expect("preserve"),
            CreateIfAbsent::AlreadyExists
        );
        assert_eq!(root.read_immutable("claim").expect("read"), b"first");
        assert_eq!(
            fs::metadata(&temp.0)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn digest_addressed_removal_recovers_after_the_quarantine_rename() {
        use sha2::{Digest, Sha256};

        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let bytes = b"object bytes";
        root.create_file_if_absent("object", bytes)
            .expect("create object");
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let digest_hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let quarantine = temp.0.join(format!(".maestro-remove-{digest_hex}.pending"));
        fs::rename(temp.0.join("object"), &quarantine)
            .expect("simulate crash after durable quarantine rename");

        assert!(
            root.finish_file_removal_if_digest_matches("object", &digest)
                .expect("recover pending removal")
        );
        assert!(!quarantine.exists());
        assert!(
            !root
                .finish_file_removal_if_digest_matches("object", &digest)
                .expect("completed removal is idempotent")
        );
    }

    #[test]
    fn nested_directories_are_descriptor_anchored() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("evidence/claims").expect("create dirs");
        let claims = root.open_dir("evidence/claims").expect("open dirs");
        claims
            .create_file_if_absent("one", b"claim")
            .expect("create file");
        assert_eq!(claims.path(), temp.0.join("evidence/claims"));
        assert_eq!(
            claims.read_exact("one", b"claim").expect("verify"),
            b"claim"
        );
    }

    #[test]
    fn regular_file_binding_verifies_the_exact_opened_file() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("claim", b"claim")
            .expect("create claim");

        let binding = root.bind_regular_file("claim").expect("bind claim");

        root.verify_regular_file_binding("claim", &binding)
            .expect("verify exact binding");
    }

    #[test]
    fn regular_file_binding_rejects_same_content_inode_substitution() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("claim", b"same bytes")
            .expect("create claim");
        let binding = root.bind_regular_file("claim").expect("bind claim");
        fs::rename(temp.0.join("claim"), temp.0.join("displaced")).expect("retain displaced inode");
        root.create_file_if_absent("claim", b"same bytes")
            .expect("replace claim with same bytes");

        assert!(matches!(
            root.verify_regular_file_binding("claim", &binding),
            Err(SecureFsError::ChangedDuringRead { .. })
        ));
    }

    #[test]
    fn optional_regular_file_binding_rejects_presence_and_identity_changes() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let absent = root
            .bind_optional_regular_file("absent")
            .expect("bind absence");
        assert!(absent.is_none());
        root.verify_optional_regular_file_binding("absent", absent.as_ref())
            .expect("verify absence");
        root.create_file_if_absent("absent", b"appeared")
            .expect("create previously absent file");
        assert!(matches!(
            root.verify_optional_regular_file_binding("absent", absent.as_ref()),
            Err(SecureFsError::ChangedDuringRead { .. })
        ));

        root.create_file_if_absent("present", b"present")
            .expect("create present file");
        let present = root
            .bind_optional_regular_file("present")
            .expect("bind presence");
        root.verify_optional_regular_file_binding("present", present.as_ref())
            .expect("verify presence");
        fs::rename(temp.0.join("present"), temp.0.join("removed"))
            .expect("remove bound leaf while retaining inode");
        assert!(matches!(
            root.verify_optional_regular_file_binding("present", present.as_ref()),
            Err(SecureFsError::ChangedDuringRead { .. })
        ));

        root.create_file_if_absent("substituted", b"same bytes")
            .expect("create substitution candidate");
        let substituted = root
            .bind_optional_regular_file("substituted")
            .expect("bind substitution candidate");
        fs::rename(temp.0.join("substituted"), temp.0.join("original"))
            .expect("retain original substitution inode");
        root.create_file_if_absent("substituted", b"same bytes")
            .expect("substitute same bytes");
        assert!(matches!(
            root.verify_optional_regular_file_binding("substituted", substituted.as_ref()),
            Err(SecureFsError::ChangedDuringRead { .. })
        ));
    }

    #[test]
    fn regular_file_bindings_refuse_symlinks_and_hard_links() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("target", b"target")
            .expect("create target");
        symlink("target", temp.0.join("linked")).expect("create symlink");

        assert!(matches!(
            root.bind_regular_file("linked"),
            Err(SecureFsError::Io { .. })
        ));
        assert!(matches!(
            root.bind_optional_regular_file("linked"),
            Err(SecureFsError::Io { .. })
        ));

        let binding = root.bind_regular_file("target").expect("bind target");
        fs::hard_link(temp.0.join("target"), temp.0.join("alias")).expect("create hard link");
        assert!(matches!(
            root.verify_regular_file_binding("target", &binding),
            Err(SecureFsError::UnsafeObject { .. })
        ));
        assert!(matches!(
            root.verify_optional_regular_file_binding("target", Some(&binding)),
            Err(SecureFsError::UnsafeObject { .. })
        ));
        assert!(matches!(
            root.bind_regular_file("alias"),
            Err(SecureFsError::UnsafeObject { .. })
        ));
    }

    #[test]
    fn rejects_escape_and_symlink_paths() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        fs::create_dir(temp.0.join("real")).expect("create dir");
        symlink(temp.0.join("real"), temp.0.join("linked")).expect("symlink");
        assert!(matches!(
            root.create_file_if_absent("/escape", b"no"),
            Err(SecureFsError::InvalidPath { .. })
        ));
        assert!(matches!(
            root.create_file_if_absent("../escape", b"no"),
            Err(SecureFsError::InvalidPath { .. })
        ));
        assert!(matches!(
            root.create_file_if_absent("linked/file", b"no"),
            Err(SecureFsError::Io { .. })
        ));
    }

    #[test]
    fn rejects_symlink_leaf_and_unsafe_root_permissions() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        fs::write(temp.0.join("target"), b"target").expect("write");
        symlink("target", temp.0.join("leaf")).expect("symlink");
        assert!(matches!(
            root.read_immutable("leaf"),
            Err(SecureFsError::Io { .. })
        ));
        drop(root);
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o770)).expect("chmod");
        assert!(matches!(
            SecureRoot::open(&temp.0),
            Err(SecureFsError::UnsafeObject { .. })
        ));
    }

    #[test]
    fn exact_match_guards_durable_removal() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("receipt", b"v1")
            .expect("create");
        assert!(matches!(
            root.remove_file_if_matches("receipt", b"stale"),
            Err(SecureFsError::ContentMismatch { .. })
        ));
        assert!(
            root.remove_file_if_matches("receipt", b"v1")
                .expect("remove")
        );
        assert!(
            !root
                .remove_file_if_matches("receipt", b"v1")
                .expect("missing")
        );
    }

    #[test]
    fn empty_directory_removal_is_idempotent() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("empty").expect("create dir");
        assert!(root.remove_empty_dir("empty").expect("remove"));
        assert!(!root.remove_empty_dir("empty").expect("missing"));
    }

    #[test]
    fn rejects_unsafe_child_permissions_before_removal() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("unsafe").expect("create dir");
        fs::set_permissions(temp.0.join("unsafe"), fs::Permissions::from_mode(0o770))
            .expect("chmod");

        assert!(matches!(
            root.remove_empty_dir("unsafe"),
            Err(SecureFsError::UnsafeObject { .. })
        ));
        assert!(temp.0.join("unsafe").is_dir());
    }

    #[test]
    fn rejects_hard_linked_immutable_files() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("object", b"immutable")
            .expect("create object");
        fs::hard_link(temp.0.join("object"), temp.0.join("alias")).expect("create hard link");

        assert!(matches!(
            root.read_immutable("object"),
            Err(SecureFsError::UnsafeObject { .. })
        ));
        assert!(matches!(
            root.read_immutable("alias"),
            Err(SecureFsError::UnsafeObject { .. })
        ));
    }

    #[test]
    fn refuses_files_beyond_the_bounded_read_limit() {
        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let file = fs::File::create(temp.0.join("oversized")).expect("create oversized file");
        file.set_len((super::MAX_FILE_BYTES as u64) + 1)
            .expect("extend oversized file");

        assert!(matches!(
            root.read_immutable("oversized"),
            Err(SecureFsError::UnsafeObject { .. })
        ));
    }
}
