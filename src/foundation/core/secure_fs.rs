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
    #[error("descriptor-anchored census refused")]
    CensusRefused,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SecureDirectoryEntryKind {
    RegularFile,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SecureDirectoryEntry {
    name: PathBuf,
    kind: SecureDirectoryEntryKind,
}

impl SecureDirectoryEntry {
    pub(crate) fn name(&self) -> &std::path::Path {
        &self.name
    }

    pub(crate) const fn kind(&self) -> SecureDirectoryEntryKind {
        self.kind
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RegularFileBinding {
    identity: platform::RegularFileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DescriptorCensusObjectKindV1 {
    RegularFile,
    SymbolicLink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorCensusLimitsV1 {
    maximum_depth: usize,
    maximum_entries: usize,
    maximum_leaf_bytes: usize,
    maximum_total_bytes: usize,
    maximum_link_target_bytes: usize,
    maximum_name_bytes: usize,
}

impl DescriptorCensusLimitsV1 {
    pub const fn bounded(
        maximum_depth: usize,
        maximum_entries: usize,
        maximum_leaf_bytes: usize,
        maximum_total_bytes: usize,
        maximum_link_target_bytes: usize,
        maximum_name_bytes: usize,
    ) -> Option<Self> {
        if maximum_depth == 0
            || maximum_depth > MAX_COMPONENTS
            || maximum_entries == 0
            || maximum_leaf_bytes == 0
            || maximum_leaf_bytes > MAX_FILE_BYTES
            || maximum_total_bytes == 0
            || maximum_link_target_bytes == 0
            || maximum_link_target_bytes > MAX_PATH_BYTES
            || maximum_name_bytes == 0
            || maximum_name_bytes > MAX_COMPONENT_BYTES
        {
            return None;
        }
        Some(Self {
            maximum_depth,
            maximum_entries,
            maximum_leaf_bytes,
            maximum_total_bytes,
            maximum_link_target_bytes,
            maximum_name_bytes,
        })
    }

    pub const fn bounded_default() -> Self {
        Self {
            maximum_depth: MAX_COMPONENTS,
            maximum_entries: 100_000,
            maximum_leaf_bytes: MAX_FILE_BYTES,
            maximum_total_bytes: MAX_FILE_BYTES * 16,
            maximum_link_target_bytes: MAX_PATH_BYTES,
            maximum_name_bytes: MAX_COMPONENT_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DescriptorCensusMutationFenceFactsV1 {
    writer_identity: [u8; 32],
    namespace_identity: [u8; 32],
    monotonic_epoch: u64,
}

impl DescriptorCensusMutationFenceFactsV1 {
    pub(in crate::foundation::core) fn from_live_owner(
        writer_identity: [u8; 32],
        namespace_identity: [u8; 32],
        monotonic_epoch: u64,
    ) -> Option<Self> {
        if writer_identity == [0; 32] || namespace_identity == [0; 32] || monotonic_epoch == 0 {
            return None;
        }
        Some(Self {
            writer_identity,
            namespace_identity,
            monotonic_epoch,
        })
    }
}

pub(in crate::foundation::core) mod descriptor_census_sealed {
    #[cfg(test)]
    pub trait FenceSealed {}
    pub trait LeaseSealed {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DescriptorCensusRootBindingV1 {
    identity: [u8; 32],
}

impl DescriptorCensusRootBindingV1 {
    fn from_descriptor_chain(identity: [u8; 32]) -> Option<Self> {
        (identity != [0; 32]).then_some(Self { identity })
    }
}

pub(crate) trait DescriptorCensusMutationLeasePortV1:
    descriptor_census_sealed::LeaseSealed
{
    fn facts(&self) -> DescriptorCensusMutationFenceFactsV1;
    fn root_binding(&self) -> DescriptorCensusRootBindingV1;
    fn consume_final_recheck(self) -> bool;
}

#[cfg(test)]
pub(crate) trait DescriptorCensusMutationFenceV1:
    descriptor_census_sealed::FenceSealed
{
    type Lease<'lease>: DescriptorCensusMutationLeasePortV1
    where
        Self: 'lease;

    fn acquire_exclusive(
        &mut self,
        root_binding: DescriptorCensusRootBindingV1,
    ) -> Option<Self::Lease<'_>>;
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VerifiedRelativeLocatorV1(Vec<u8>);

impl VerifiedRelativeLocatorV1 {
    fn from_lossless_bounded_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InventoryRowV1 {
    locator: VerifiedRelativeLocatorV1,
    kind: DescriptorCensusObjectKindV1,
    logical_byte_length: u64,
    object_identity: [u8; 32],
    content_identity: [u8; 32],
}

impl InventoryRowV1 {
    pub fn relative_name(&self) -> &[u8] {
        self.locator.as_bytes()
    }

    pub const fn kind(&self) -> DescriptorCensusObjectKindV1 {
        self.kind
    }

    pub const fn logical_byte_length(&self) -> u64 {
        self.logical_byte_length
    }

    pub const fn object_identity(&self) -> [u8; 32] {
        self.object_identity
    }

    pub const fn content_identity(&self) -> [u8; 32] {
        self.content_identity
    }
}

#[derive(Eq, PartialEq)]
pub struct DescriptorAnchoredCensusV1 {
    rows: Vec<InventoryRowV1>,
    identity: [u8; 32],
}

impl DescriptorAnchoredCensusV1 {
    pub fn rows(&self) -> &[InventoryRowV1] {
        &self.rows
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
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
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Component, Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sha2::{Digest, Sha256};

    #[cfg(test)]
    use super::DescriptorCensusMutationFenceV1;
    use super::{
        CreateIfAbsent, DescriptorAnchoredCensusV1, DescriptorCensusLimitsV1,
        DescriptorCensusMutationFenceFactsV1, DescriptorCensusMutationLeasePortV1,
        DescriptorCensusObjectKindV1, DescriptorCensusRootBindingV1, InventoryRowV1,
        MAX_COMPONENT_BYTES, MAX_COMPONENTS, MAX_PATH_BYTES, RegularFileBinding,
        SecureDirectoryEntry, SecureDirectoryEntryKind, SecureFsError, SecureFsResult,
        VerifiedRelativeLocatorV1,
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
    static NAMESPACE_EPOCH: AtomicU64 = AtomicU64::new(1);

    #[cfg(test)]
    type RemovalUnlinkHook = Option<Box<dyn FnOnce(&Path)>>;
    #[cfg(test)]
    type CensusPassHook = Option<Box<dyn FnOnce(&Path)>>;

    #[cfg(test)]
    thread_local! {
        static BEFORE_REMOVAL_UNLINK_TEST_HOOK:
            std::cell::RefCell<RemovalUnlinkHook> = std::cell::RefCell::new(None);
        static AFTER_REMOVAL_SENTINEL_CHECK_TEST_HOOK:
            std::cell::RefCell<RemovalUnlinkHook> = std::cell::RefCell::new(None);
        static FORCE_PLATFORM_CENSUS_FENCE_LOSS: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
        static AFTER_FIRST_CENSUS_PASS_TEST_HOOK:
            std::cell::RefCell<CensusPassHook> = std::cell::RefCell::new(None);
    }

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
        pub const O_NONBLOCK: c_int = 0o4000;
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
        pub const O_NONBLOCK: c_int = 0x4;
        pub const AT_REMOVEDIR: c_int = 0x80;
    }

    #[repr(C)]
    struct DirectoryStream {
        _private: [u8; 0],
    }

    #[cfg(target_os = "linux")]
    #[repr(C)]
    struct DirectoryEntryRecord {
        d_ino: u64,
        d_off: i64,
        d_reclen: u16,
        d_type: u8,
        d_name: [c_char; 256],
    }

    #[cfg(target_os = "macos")]
    #[repr(C)]
    struct DirectoryEntryRecord {
        d_ino: u64,
        d_seekoff: u64,
        d_reclen: u16,
        d_namlen: u16,
        d_type: u8,
        d_name: [c_char; 1024],
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
        fn readlinkat(
            directory: c_int,
            path: *const c_char,
            buffer: *mut c_char,
            size: usize,
        ) -> isize;
        #[cfg(target_os = "linux")]
        fn flistxattr(descriptor: c_int, list: *mut c_char, size: usize) -> isize;
        #[cfg(target_os = "macos")]
        fn flistxattr(descriptor: c_int, list: *mut c_char, size: usize, options: c_int) -> isize;
        fn linkat(
            old_directory: c_int,
            old_path: *const c_char,
            new_directory: c_int,
            new_path: *const c_char,
            flags: c_int,
        ) -> c_int;
        fn geteuid() -> c_uint;
        #[cfg(any(
            target_os = "linux",
            all(target_os = "macos", not(target_arch = "x86_64"))
        ))]
        fn fdopendir(descriptor: c_int) -> *mut DirectoryStream;
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        #[link_name = "fdopendir$INODE64"]
        fn fdopendir(descriptor: c_int) -> *mut DirectoryStream;
        #[cfg(any(
            target_os = "linux",
            all(target_os = "macos", not(target_arch = "x86_64"))
        ))]
        fn readdir(stream: *mut DirectoryStream) -> *mut DirectoryEntryRecord;
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        #[link_name = "readdir$INODE64"]
        fn readdir(stream: *mut DirectoryStream) -> *mut DirectoryEntryRecord;
        fn closedir(stream: *mut DirectoryStream) -> c_int;
        fn flock(descriptor: c_int, operation: c_int) -> c_int;
        #[cfg(target_os = "linux")]
        fn __errno_location() -> *mut c_int;
        #[cfg(target_os = "macos")]
        fn __error() -> *mut c_int;
        #[cfg(target_os = "macos")]
        fn fstatfs(descriptor: c_int, buffer: *mut DarwinStatFs) -> c_int;
    }

    #[cfg(target_os = "macos")]
    #[repr(C)]
    struct DarwinStatFs {
        f_bsize: u32,
        f_iosize: i32,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_fsid: [i32; 2],
        f_owner: u32,
        f_type: u32,
        f_flags: u32,
        f_fssubtype: u32,
        f_fstypename: [c_char; 16],
        f_mntonname: [c_char; 1024],
        f_mntfromname: [c_char; 1024],
        f_flags_ext: u32,
        f_reserved: [u32; 7],
    }

    #[derive(Debug)]
    struct RetainedDescriptorEdgeV1 {
        parent: File,
        child_name: CString,
    }

    #[derive(Debug)]
    pub struct SecureRoot {
        directory: File,
        namespace_anchor: File,
        descriptor_chain: Vec<RetainedDescriptorEdgeV1>,
        path: PathBuf,
    }

    const LOCK_EXCLUSIVE: c_int = 2;
    const LOCK_UNLOCK: c_int = 8;

    struct NamespaceMutationLeaseV1 {
        descriptor: File,
    }

    impl NamespaceMutationLeaseV1 {
        fn acquire(anchor: &File, path: &Path) -> SecureFsResult<Self> {
            let descriptor = open_directory_at_unchecked(anchor.as_raw_fd(), c".", path)?;
            if unsafe { flock(descriptor.as_raw_fd(), LOCK_EXCLUSIVE) } != 0 {
                return Err(SecureFsError::Io {
                    operation: "acquire secure namespace mutation fence",
                    path: path.to_path_buf(),
                    source: io::Error::last_os_error(),
                });
            }
            NAMESPACE_EPOCH.fetch_add(1, Ordering::AcqRel);
            Ok(Self { descriptor })
        }
    }

    impl Drop for NamespaceMutationLeaseV1 {
        fn drop(&mut self) {
            unsafe {
                flock(self.descriptor.as_raw_fd(), LOCK_UNLOCK);
            }
        }
    }

    struct PlatformCensusLeaseV1 {
        lease: NamespaceMutationLeaseV1,
        facts: DescriptorCensusMutationFenceFactsV1,
        root_binding: DescriptorCensusRootBindingV1,
    }

    impl super::descriptor_census_sealed::LeaseSealed for PlatformCensusLeaseV1 {}

    pub struct AdmittedDescriptorCensusRootV1<'root> {
        root: &'root SecureRoot,
        initial_root: FileIdentity,
        root_mount: [u8; 32],
        root_binding: DescriptorCensusRootBindingV1,
        lease: PlatformCensusLeaseV1,
    }

    impl DescriptorCensusMutationLeasePortV1 for PlatformCensusLeaseV1 {
        fn facts(&self) -> DescriptorCensusMutationFenceFactsV1 {
            self.facts
        }

        fn root_binding(&self) -> DescriptorCensusRootBindingV1 {
            self.root_binding
        }

        fn consume_final_recheck(self) -> bool {
            let _held_until_return = self.lease;
            #[cfg(test)]
            if FORCE_PLATFORM_CENSUS_FENCE_LOSS.with(std::cell::Cell::take) {
                return false;
            }
            true
        }
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
            for (index, edge) in self.descriptor_chain.iter().enumerate() {
                let observed = open_directory_at_unchecked(
                    edge.parent.as_raw_fd(),
                    &edge.child_name,
                    &self.path,
                )?;
                let expected = self
                    .descriptor_chain
                    .get(index + 1)
                    .map(|next| &next.parent)
                    .unwrap_or(&self.directory);
                if ObjectIdentity::from(&metadata(&observed, &self.path)?)
                    != ObjectIdentity::from(&metadata(expected, &self.path)?)
                {
                    return Err(SecureFsError::ChangedDuringRead {
                        path: self.path.clone(),
                    });
                }
            }
            Ok(())
        }

        #[cfg(test)]
        pub(crate) fn install_before_removal_unlink_test_hook(hook: impl FnOnce(&Path) + 'static) {
            BEFORE_REMOVAL_UNLINK_TEST_HOOK.with(|slot| {
                assert!(
                    slot.borrow_mut().replace(Box::new(hook)).is_none(),
                    "file-removal unlink hook must be exclusive"
                );
            });
        }

        #[cfg(test)]
        pub(crate) fn install_after_removal_sentinel_check_test_hook(
            hook: impl FnOnce(&Path) + 'static,
        ) {
            AFTER_REMOVAL_SENTINEL_CHECK_TEST_HOOK.with(|slot| {
                assert!(
                    slot.borrow_mut().replace(Box::new(hook)).is_none(),
                    "file-removal sentinel hook must be exclusive"
                );
            });
        }

        pub fn open_dir(&self, relative: impl AsRef<Path>) -> SecureFsResult<Self> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let mut directory = self.clone_directory()?;
            let mut descriptor_chain = clone_descriptor_chain(&self.descriptor_chain, &self.path)?;
            for component in &relative.components {
                let next = open_directory_at(
                    directory.as_raw_fd(),
                    component,
                    &self.path.join(&relative.path),
                )?;
                descriptor_chain.push(RetainedDescriptorEdgeV1 {
                    parent: directory,
                    child_name: component.clone(),
                });
                directory = next;
            }
            let path = self.path.join(relative.path);
            validate_directory(&directory, &path)?;
            Ok(Self {
                directory,
                namespace_anchor: open_directory_at_unchecked(
                    self.namespace_anchor.as_raw_fd(),
                    c".",
                    &self.path,
                )?,
                descriptor_chain,
                path,
            })
        }

        pub fn create_dir_all(&self, relative: impl AsRef<Path>) -> SecureFsResult<()> {
            let _namespace = self.acquire_namespace_mutation_lease()?;
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

        pub(crate) fn read_dir_entries(&self) -> SecureFsResult<Vec<SecureDirectoryEntry>> {
            descriptor_directory_entries(self)
        }

        #[cfg(test)]
        pub(crate) fn descriptor_anchored_census<F: DescriptorCensusMutationFenceV1>(
            &self,
            limits: DescriptorCensusLimitsV1,
            fence: &mut F,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
            self.descriptor_anchored_census_inner(limits, fence)
                .map_err(|_| SecureFsError::CensusRefused)
        }

        pub(in crate::foundation::core) fn admit_descriptor_census_root(
            &self,
        ) -> SecureFsResult<AdmittedDescriptorCensusRootV1<'_>> {
            let initial_root = FileIdentity::from(&metadata(&self.directory, &self.path)?);
            let root_mount = mount_identity(&self.directory, &self.path)?;
            let root_binding = self.descriptor_chain_binding(root_mount)?;
            let lease = NamespaceMutationLeaseV1::acquire(&self.namespace_anchor, &self.path)?;
            let mut writer = Sha256::new();
            writer.update(b"maestro.foundation.namespace-writer.v1\0");
            writer.update(process::id().to_be_bytes());
            writer.update(root_binding.identity);
            let lease = PlatformCensusLeaseV1 {
                lease,
                facts: DescriptorCensusMutationFenceFactsV1 {
                    writer_identity: writer.finalize().into(),
                    namespace_identity: root_binding.identity,
                    monotonic_epoch: NAMESPACE_EPOCH.fetch_add(1, Ordering::AcqRel),
                },
                root_binding,
            };
            Ok(AdmittedDescriptorCensusRootV1 {
                root: self,
                initial_root,
                root_mount,
                root_binding,
                lease,
            })
        }

        pub(in crate::foundation::core) fn census_admitted_descriptor_root(
            admitted: AdmittedDescriptorCensusRootV1<'_>,
            limits: DescriptorCensusLimitsV1,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
            let AdmittedDescriptorCensusRootV1 {
                root,
                initial_root,
                root_mount,
                root_binding,
                lease,
            } = admitted;
            root.descriptor_anchored_census_with_lease(
                limits,
                initial_root,
                root_mount,
                root_binding,
                lease,
            )
        }

        #[cfg(test)]
        pub(crate) fn force_platform_census_fence_loss_for_test() {
            FORCE_PLATFORM_CENSUS_FENCE_LOSS.with(|slot| slot.set(true));
        }

        #[cfg(test)]
        pub(crate) fn install_after_first_census_pass_test_hook(
            hook: impl FnOnce(&Path) + 'static,
        ) {
            AFTER_FIRST_CENSUS_PASS_TEST_HOOK.with(|slot| {
                assert!(
                    slot.borrow_mut().replace(Box::new(hook)).is_none(),
                    "descriptor census first-pass hook must be exclusive"
                );
            });
        }

        #[cfg(test)]
        fn descriptor_anchored_census_inner<F: DescriptorCensusMutationFenceV1>(
            &self,
            limits: DescriptorCensusLimitsV1,
            fence: &mut F,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
            let initial_root = FileIdentity::from(&metadata(&self.directory, &self.path)?);
            let root_mount = mount_identity(&self.directory, &self.path)?;
            let root_binding = self.descriptor_chain_binding(root_mount)?;
            let lease = fence.acquire_exclusive(root_binding).ok_or_else(|| {
                SecureFsError::UnsafeObject {
                    path: self.path.clone(),
                    reason: "descriptor census namespace-writer lease is unavailable",
                }
            })?;
            self.descriptor_anchored_census_with_lease(
                limits,
                initial_root,
                root_mount,
                root_binding,
                lease,
            )
        }

        fn descriptor_anchored_census_with_lease<L: DescriptorCensusMutationLeasePortV1>(
            &self,
            limits: DescriptorCensusLimitsV1,
            initial_root: FileIdentity,
            root_mount: [u8; 32],
            root_binding: DescriptorCensusRootBindingV1,
            lease: L,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
            let initial_fence = lease.facts();
            if DescriptorCensusMutationFenceFactsV1::from_live_owner(
                initial_fence.writer_identity,
                initial_fence.namespace_identity,
                initial_fence.monotonic_epoch,
            ) != Some(initial_fence)
                || lease.root_binding() != root_binding
            {
                return Err(SecureFsError::UnsafeObject {
                    path: self.path.clone(),
                    reason: "descriptor census namespace-writer lease is invalid",
                });
            }
            ensure_no_semantic_metadata(&self.directory, &self.path)?;
            let first = descriptor_census_pass(self, initial_root.device, root_mount, limits)?;
            #[cfg(test)]
            AFTER_FIRST_CENSUS_PASS_TEST_HOOK.with(|slot| {
                if let Some(hook) = slot.borrow_mut().take() {
                    hook(&self.path);
                }
            });
            if self.descriptor_chain_binding(root_mount)? != root_binding {
                return Err(SecureFsError::ChangedDuringRead {
                    path: self.path.clone(),
                });
            }
            let second_root = FileIdentity::from(&metadata(&self.directory, &self.path)?);
            let second = descriptor_census_pass(self, initial_root.device, root_mount, limits)?;
            let final_root = FileIdentity::from(&metadata(&self.directory, &self.path)?);
            let final_root_binding = self.descriptor_chain_binding(root_mount)?;
            if initial_root != second_root
                || initial_root != final_root
                || first != second
                || final_root_binding != root_binding
                || !lease.consume_final_recheck()
            {
                return Err(SecureFsError::ChangedDuringRead {
                    path: self.path.clone(),
                });
            }
            if first
                .windows(2)
                .any(|rows| rows[0].locator == rows[1].locator)
            {
                return Err(SecureFsError::UnsafeObject {
                    path: self.path.clone(),
                    reason: "descriptor census contains a duplicate relative locator",
                });
            }
            let mut object_identities = first
                .iter()
                .map(|row| row.object_identity)
                .collect::<Vec<_>>();
            object_identities.sort_unstable();
            if object_identities.windows(2).any(|rows| rows[0] == rows[1]) {
                return Err(SecureFsError::UnsafeObject {
                    path: self.path.clone(),
                    reason: "descriptor census contains an object identity alias",
                });
            }
            let mut hasher = Sha256::new();
            hasher.update(b"maestro.foundation.descriptor-anchored-census.v1\0");
            hasher.update(initial_fence.writer_identity);
            hasher.update(initial_fence.namespace_identity);
            hasher.update(initial_fence.monotonic_epoch.to_be_bytes());
            for row in &first {
                hasher.update((row.locator.as_bytes().len() as u64).to_be_bytes());
                hasher.update(row.locator.as_bytes());
                hasher.update([match row.kind {
                    DescriptorCensusObjectKindV1::RegularFile => 1,
                    DescriptorCensusObjectKindV1::SymbolicLink => 2,
                }]);
                hasher.update(row.logical_byte_length.to_be_bytes());
                hasher.update(row.object_identity);
                hasher.update(row.content_identity);
            }
            Ok(DescriptorAnchoredCensusV1 {
                rows: first,
                identity: hasher.finalize().into(),
            })
        }

        fn descriptor_chain_binding(
            &self,
            root_mount: [u8; 32],
        ) -> SecureFsResult<DescriptorCensusRootBindingV1> {
            self.verify_path_binding()?;
            let mut hasher = Sha256::new();
            hasher.update(b"maestro.foundation.descriptor-root-binding.v1\0");
            hasher.update(root_mount);
            for (index, edge) in self.descriptor_chain.iter().enumerate() {
                let expected = self
                    .descriptor_chain
                    .get(index + 1)
                    .map(|next| &next.parent)
                    .unwrap_or(&self.directory);
                let parent = ObjectIdentity::from(&metadata(&edge.parent, &self.path)?);
                let child = ObjectIdentity::from(&metadata(expected, &self.path)?);
                update_object_identity(&mut hasher, parent);
                hasher.update((edge.child_name.to_bytes().len() as u64).to_be_bytes());
                hasher.update(edge.child_name.to_bytes());
                update_object_identity(&mut hasher, child);
            }
            let root = ObjectIdentity::from(&metadata(&self.directory, &self.path)?);
            update_object_identity(&mut hasher, root);
            DescriptorCensusRootBindingV1::from_descriptor_chain(hasher.finalize().into())
                .ok_or_else(|| unsafe_object(&self.path, "descriptor root binding is invalid"))
        }

        pub fn create_file_if_absent(
            &self,
            relative: impl AsRef<Path>,
            contents: &[u8],
        ) -> SecureFsResult<CreateIfAbsent> {
            let _namespace = self.acquire_namespace_mutation_lease()?;
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
            let _namespace = self.acquire_namespace_mutation_lease()?;
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
            let _namespace = self.acquire_namespace_mutation_lease()?;
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let quarantine = removal_quarantine_name(expected);
            let digest: [u8; 32] = Sha256::digest(expected).into();
            let state = RemovalState::new(leaf, &digest, quarantine);
            let mut file = match open_regular_file_at(&parent, leaf, &path) {
                Ok(file) => file,
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                    return finish_quarantined_removal(&parent, &state, expected, &path);
                }
                Err(error) => return Err(error),
            };
            validate_regular_file(&file, &path)?;
            let opened_identity = ObjectIdentity::from(&metadata(&file, &path)?);
            if read_stable(&mut file, &path)? != expected {
                return Err(SecureFsError::ContentMismatch { path });
            }
            move_to_removal_quarantine(&parent, leaf, &state.quarantine, &path)?;
            let verification = (|| {
                let candidate = open_regular_file_at(&parent, &state.quarantine, &path)?;
                validate_regular_file(&candidate, &path)?;
                if ObjectIdentity::from(&metadata(&candidate, &path)?) != opened_identity {
                    return Err(SecureFsError::ChangedDuringRead { path: path.clone() });
                }
                Ok(())
            })();
            if let Err(error) = verification {
                restore_from_quarantine(&parent, &state.quarantine, leaf, &path)?;
                return Err(error);
            }
            finish_open_quarantined_removal(&parent, &state, &file, &path)
        }

        pub fn finish_file_removal_if_digest_matches(
            &self,
            relative: impl AsRef<Path>,
            expected_digest: &[u8; 32],
        ) -> SecureFsResult<bool> {
            let _namespace = self.acquire_namespace_mutation_lease()?;
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent(&relative)?;
            let state = RemovalState::new(
                leaf,
                expected_digest,
                removal_quarantine_name_for_digest(expected_digest),
            );
            finish_quarantined_removal_by_digest(&parent, &state, expected_digest, &path)
        }

        pub(crate) fn verify_file_removal_resolved(
            &self,
            relative: impl AsRef<Path>,
            expected_digest: &[u8; 32],
        ) -> SecureFsResult<()> {
            let relative = BoundedPath::new(relative.as_ref())?;
            let (parent, leaf, path) = self.open_parent_root(&relative)?;
            let state = RemovalState::new(
                leaf,
                expected_digest,
                removal_quarantine_name_for_digest(expected_digest),
            );
            match open_regular_file_at(&parent.directory, &state.debt, &path) {
                Ok(mut marker) => {
                    validate_regular_file(&marker, &path)?;
                    if read_stable(&mut marker, &path)? != state.debt_bytes {
                        return Err(SecureFsError::ContentMismatch { path });
                    }
                    return Err(SecureFsError::UnsafeObject {
                        path: path.clone(),
                        reason: "file removal has unresolved hard-link or crash debt",
                    });
                }
                Err(error) if error.io_kind(io::ErrorKind::NotFound) => {}
                Err(error) => return Err(error),
            }
            verify_removal_state_absent(&parent.directory, &state, &path)?;
            parent.verify_no_crash_residual_temp_by_digest(Path::new("."), expected_digest)
        }

        pub(crate) fn remove_crash_residual_temps_by_digest(
            &self,
            relative_directory: impl AsRef<Path>,
            expected_digest: &[u8; 32],
        ) -> SecureFsResult<usize> {
            let directory = self.open_dir(relative_directory)?;
            let mut removed = 0;
            for name in secure_temp_file_names(&directory)? {
                let bytes = directory.read_immutable(&name)?;
                let observed: [u8; 32] = Sha256::digest(&bytes).into();
                if &observed == expected_digest {
                    directory.remove_file_if_matches(&name, &bytes)?;
                    removed += 1;
                }
            }
            directory.verify_no_crash_residual_temp_by_digest(Path::new("."), expected_digest)?;
            Ok(removed)
        }

        pub(crate) fn verify_no_crash_residual_temp_by_digest(
            &self,
            relative_directory: impl AsRef<Path>,
            expected_digest: &[u8; 32],
        ) -> SecureFsResult<()> {
            let relative = relative_directory.as_ref();
            let directory = if relative == Path::new(".") {
                Self {
                    directory: self.clone_directory()?,
                    namespace_anchor: open_directory_at_unchecked(
                        self.namespace_anchor.as_raw_fd(),
                        c".",
                        &self.path,
                    )?,
                    descriptor_chain: clone_descriptor_chain(&self.descriptor_chain, &self.path)?,
                    path: self.path.clone(),
                }
            } else {
                self.open_dir(relative)?
            };
            for name in secure_temp_file_names(&directory)? {
                let bytes = directory.read_immutable(&name)?;
                let observed: [u8; 32] = Sha256::digest(&bytes).into();
                if &observed == expected_digest {
                    return Err(SecureFsError::UnsafeObject {
                        path: directory.path.join(name),
                        reason: "crash-residual temporary file still carries removed bytes",
                    });
                }
            }
            Ok(())
        }

        pub fn remove_empty_dir(&self, relative: impl AsRef<Path>) -> SecureFsResult<bool> {
            let _namespace = self.acquire_namespace_mutation_lease()?;
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
            let (parent, leaf, path) = self.open_parent_root(relative)?;
            Ok((parent.directory, leaf, path))
        }

        fn open_parent_root<'a>(
            &self,
            relative: &'a BoundedPath,
        ) -> SecureFsResult<(Self, &'a CStr, PathBuf)> {
            let (leaf, parents) = relative
                .components
                .split_last()
                .expect("invariant: bounded path has a component");
            let mut directory = self.clone_directory()?;
            let mut descriptor_chain = clone_descriptor_chain(&self.descriptor_chain, &self.path)?;
            let mut traversed = PathBuf::new();
            for component in parents {
                traversed.push(OsStr::from_bytes(component.to_bytes()));
                let next = open_directory_at(
                    directory.as_raw_fd(),
                    component,
                    &self.path.join(&traversed),
                )?;
                descriptor_chain.push(RetainedDescriptorEdgeV1 {
                    parent: directory,
                    child_name: component.clone(),
                });
                directory = next;
            }
            let parent_path = self.path.join(&traversed);
            Ok((
                Self {
                    directory,
                    namespace_anchor: open_directory_at_unchecked(
                        self.namespace_anchor.as_raw_fd(),
                        c".",
                        &self.path,
                    )?,
                    descriptor_chain,
                    path: parent_path,
                },
                leaf,
                self.path.join(&relative.path),
            ))
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

        fn acquire_namespace_mutation_lease(&self) -> SecureFsResult<NamespaceMutationLeaseV1> {
            NamespaceMutationLeaseV1::acquire(&self.namespace_anchor, &self.path)
        }
    }

    fn secure_temp_file_names(root: &SecureRoot) -> SecureFsResult<Vec<PathBuf>> {
        let mut names = Vec::new();
        for entry in root.read_dir_entries()? {
            let bytes = entry.name().as_os_str().as_bytes();
            if !is_secure_temp_name(bytes) {
                continue;
            }
            if entry.kind() != SecureDirectoryEntryKind::RegularFile {
                return Err(SecureFsError::UnsafeObject {
                    path: root.path.join(entry.name()),
                    reason: "secure temporary carrier is not a regular file",
                });
            }
            names.push(entry.name().to_path_buf());
        }
        names.sort();
        Ok(names)
    }

    struct OwnedDirectoryStream(*mut DirectoryStream);

    impl Drop for OwnedDirectoryStream {
        fn drop(&mut self) {
            unsafe {
                closedir(self.0);
            }
        }
    }

    fn descriptor_directory_entries(
        root: &SecureRoot,
    ) -> SecureFsResult<Vec<SecureDirectoryEntry>> {
        let dot = c".";
        let directory = open_directory_at_unchecked(root.directory.as_raw_fd(), dot, &root.path)?;
        let descriptor = directory.into_raw_fd();
        let stream = unsafe { fdopendir(descriptor) };
        if stream.is_null() {
            let source = io::Error::last_os_error();
            unsafe {
                drop(File::from_raw_fd(descriptor));
            }
            return Err(SecureFsError::Io {
                operation: "open descriptor-anchored directory stream",
                path: root.path.clone(),
                source,
            });
        }
        let stream = OwnedDirectoryStream(stream);
        let mut entries = Vec::new();
        loop {
            unsafe {
                *errno_location() = 0;
            }
            let record = unsafe { readdir(stream.0) };
            if record.is_null() {
                let errno = unsafe { *errno_location() };
                if errno != 0 {
                    return Err(SecureFsError::Io {
                        operation: "enumerate descriptor-anchored directory",
                        path: root.path.clone(),
                        source: io::Error::from_raw_os_error(errno),
                    });
                }
                break;
            }
            let name = unsafe { directory_entry_name(&*record) }?;
            if name == b"." || name == b".." {
                continue;
            }
            if name.is_empty() || name.len() > MAX_COMPONENT_BYTES || name.contains(&0) {
                return Err(SecureFsError::InvalidPath {
                    path: root.path.clone(),
                    reason: "directory entry name is not a bounded path component",
                });
            }
            let component = CString::new(name).map_err(|_| SecureFsError::InvalidPath {
                path: root.path.clone(),
                reason: "directory entry name contains a nul byte",
            })?;
            let path = root.path.join(OsStr::from_bytes(name));
            let descriptor = unsafe {
                openat(
                    root.directory.as_raw_fd(),
                    component.as_ptr(),
                    flags::O_RDONLY | flags::O_NOFOLLOW | flags::O_CLOEXEC | flags::O_NONBLOCK,
                )
            };
            let file = descriptor_to_file(
                descriptor,
                "open descriptor-anchored directory entry",
                &path,
            )?;
            let metadata = metadata(&file, &path)?;
            let kind = if metadata.is_file() {
                validate_regular_file(&file, &path)?;
                SecureDirectoryEntryKind::RegularFile
            } else if metadata.is_dir() {
                validate_directory(&file, &path)?;
                SecureDirectoryEntryKind::Directory
            } else {
                return Err(SecureFsError::UnsafeObject {
                    path,
                    reason: "directory entry is neither a regular file nor a directory",
                });
            };
            entries.push(SecureDirectoryEntry {
                name: PathBuf::from(OsStr::from_bytes(name)),
                kind,
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn descriptor_census_pass(
        root: &SecureRoot,
        root_device: u64,
        root_mount: [u8; 32],
        limits: DescriptorCensusLimitsV1,
    ) -> SecureFsResult<Vec<InventoryRowV1>> {
        let mut state = DescriptorCensusTraversalV1 {
            root_device,
            root_mount,
            limits,
            total_bytes: 0,
            entries_seen: 0,
            rows: Vec::new(),
        };
        descriptor_census_directory(&root.directory, &root.path, &[], 0, &mut state)?;
        state.rows.sort();
        Ok(state.rows)
    }

    struct DescriptorCensusTraversalV1 {
        root_device: u64,
        root_mount: [u8; 32],
        limits: DescriptorCensusLimitsV1,
        total_bytes: usize,
        entries_seen: usize,
        rows: Vec<InventoryRowV1>,
    }

    fn descriptor_census_directory(
        directory: &File,
        display_path: &Path,
        relative_prefix: &[u8],
        depth: usize,
        state: &mut DescriptorCensusTraversalV1,
    ) -> SecureFsResult<()> {
        if depth > state.limits.maximum_depth {
            return Err(SecureFsError::InvalidPath {
                path: display_path.to_path_buf(),
                reason: "census path exceeds the configured depth limit",
            });
        }
        let initial_directory = FileIdentity::from(&metadata(directory, display_path)?);
        if initial_directory.device != state.root_device {
            return Err(unsafe_object(
                display_path,
                "census directory crosses the root mount",
            ));
        }
        if mount_identity(directory, display_path)? != state.root_mount {
            return Err(unsafe_object(
                display_path,
                "census directory crosses the root mount identity",
            ));
        }
        let dot = c".";
        let stream_directory =
            open_directory_at_unchecked(directory.as_raw_fd(), dot, display_path)?;
        let descriptor = stream_directory.into_raw_fd();
        let stream = unsafe { fdopendir(descriptor) };
        if stream.is_null() {
            let source = io::Error::last_os_error();
            unsafe {
                drop(File::from_raw_fd(descriptor));
            }
            return Err(SecureFsError::Io {
                operation: "open descriptor-anchored census stream",
                path: display_path.to_path_buf(),
                source,
            });
        }
        let stream = OwnedDirectoryStream(stream);
        let mut names = Vec::new();
        loop {
            unsafe {
                *errno_location() = 0;
            }
            let record = unsafe { readdir(stream.0) };
            if record.is_null() {
                let errno = unsafe { *errno_location() };
                if errno != 0 {
                    return Err(SecureFsError::Io {
                        operation: "enumerate descriptor-anchored census",
                        path: display_path.to_path_buf(),
                        source: io::Error::from_raw_os_error(errno),
                    });
                }
                break;
            }
            let name = unsafe { directory_entry_name(&*record) }?;
            if name == b"." || name == b".." {
                continue;
            }
            if name.is_empty()
                || name.len() > state.limits.maximum_name_bytes.min(MAX_COMPONENT_BYTES)
                || name.contains(&0)
            {
                return Err(SecureFsError::InvalidPath {
                    path: display_path.to_path_buf(),
                    reason: "census entry is not a lossless bounded name",
                });
            }
            state.entries_seen = state
                .entries_seen
                .checked_add(1)
                .ok_or_else(|| unsafe_object(display_path, "census entry count overflowed"))?;
            if state.entries_seen > state.limits.maximum_entries {
                return Err(unsafe_object(
                    display_path,
                    "census exceeds the configured entry limit",
                ));
            }
            names.push(name.to_vec());
        }
        names.sort();
        drop(stream);

        for name in names {
            let component = CString::new(name.clone()).map_err(|_| SecureFsError::InvalidPath {
                path: display_path.to_path_buf(),
                reason: "census entry contains a nul byte",
            })?;
            let path = display_path.join(OsStr::from_bytes(&name));
            let mut relative = relative_prefix.to_vec();
            let component_count = relative_prefix.iter().filter(|byte| **byte == b'/').count()
                + usize::from(!relative_prefix.is_empty())
                + 1;
            let required_bytes = relative.len() + usize::from(!relative.is_empty()) + name.len();
            if component_count > state.limits.maximum_depth
                || component_count > MAX_COMPONENTS
                || required_bytes > MAX_PATH_BYTES
            {
                return Err(SecureFsError::InvalidPath {
                    path,
                    reason: "census path exceeds the bounded relative-name grammar",
                });
            }
            if !relative.is_empty() {
                relative.push(b'/');
            }
            relative.extend_from_slice(&name);

            if read_link_at(directory.as_raw_fd(), &component, &path)?.is_some() {
                let link = open_symlink_at(directory.as_raw_fd(), &component, &path)?;
                let before = FileIdentity::from(&metadata(&link, &path)?);
                let target = read_link_at(directory.as_raw_fd(), &component, &path)?
                    .ok_or_else(|| unsafe_object(&path, "symbolic link changed during read"))?;
                if target.len() > state.limits.maximum_link_target_bytes {
                    return Err(unsafe_object(
                        &path,
                        "symbolic-link target exceeds the configured byte limit",
                    ));
                }
                let after = FileIdentity::from(&metadata(&link, &path)?);
                if before != after {
                    return Err(SecureFsError::ChangedDuringRead { path });
                }
                validate_census_leaf(&path, after, state.root_device)?;
                if mount_identity(&link, &path)? != state.root_mount {
                    return Err(unsafe_object(
                        &path,
                        "census link crosses the root mount identity",
                    ));
                }
                ensure_no_semantic_metadata(&link, &path)?;
                state.total_bytes =
                    state.total_bytes.checked_add(target.len()).ok_or_else(|| {
                        unsafe_object(&path, "census total logical byte count overflowed")
                    })?;
                if state.total_bytes > state.limits.maximum_total_bytes {
                    return Err(unsafe_object(
                        &path,
                        "census exceeds the configured total logical byte limit",
                    ));
                }
                state.rows.push(InventoryRowV1 {
                    locator: VerifiedRelativeLocatorV1::from_lossless_bounded_bytes(relative),
                    kind: DescriptorCensusObjectKindV1::SymbolicLink,
                    logical_byte_length: target.len() as u64,
                    object_identity: census_object_identity(after),
                    content_identity: Sha256::digest(target).into(),
                });
                continue;
            }

            let descriptor = unsafe {
                openat(
                    directory.as_raw_fd(),
                    component.as_ptr(),
                    flags::O_RDONLY | flags::O_NOFOLLOW | flags::O_CLOEXEC | flags::O_NONBLOCK,
                )
            };
            let mut object =
                descriptor_to_file(descriptor, "open descriptor-anchored census entry", &path)?;
            let identity = FileIdentity::from(&metadata(&object, &path)?);
            if identity.device != state.root_device {
                return Err(unsafe_object(&path, "census entry crosses the root mount"));
            }
            if mount_identity(&object, &path)? != state.root_mount {
                return Err(unsafe_object(
                    &path,
                    "census entry crosses the root mount identity",
                ));
            }
            let object_metadata = metadata(&object, &path)?;
            if object_metadata.is_dir() {
                validate_directory(&object, &path)?;
                ensure_no_semantic_metadata(&object, &path)?;
                descriptor_census_directory(&object, &path, &relative, depth + 1, state)?;
            } else if object_metadata.is_file() {
                validate_census_leaf(&path, identity, state.root_device)?;
                ensure_no_semantic_metadata(&object, &path)?;
                let content = read_stable(&mut object, &path)?;
                if content.len() > state.limits.maximum_leaf_bytes {
                    return Err(unsafe_object(
                        &path,
                        "census leaf exceeds the configured logical byte limit",
                    ));
                }
                state.total_bytes =
                    state
                        .total_bytes
                        .checked_add(content.len())
                        .ok_or_else(|| {
                            unsafe_object(&path, "census total logical byte count overflowed")
                        })?;
                if state.total_bytes > state.limits.maximum_total_bytes {
                    return Err(unsafe_object(
                        &path,
                        "census exceeds the configured total logical byte limit",
                    ));
                }
                state.rows.push(InventoryRowV1 {
                    locator: VerifiedRelativeLocatorV1::from_lossless_bounded_bytes(relative),
                    kind: DescriptorCensusObjectKindV1::RegularFile,
                    logical_byte_length: content.len() as u64,
                    object_identity: census_object_identity(identity),
                    content_identity: Sha256::digest(content).into(),
                });
            } else {
                return Err(unsafe_object(
                    &path,
                    "census entry is not a directory, regular file, or symbolic link",
                ));
            }
        }
        if FileIdentity::from(&metadata(directory, display_path)?) != initial_directory {
            return Err(SecureFsError::ChangedDuringRead {
                path: display_path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn validate_census_leaf(
        path: &Path,
        identity: FileIdentity,
        root_device: u64,
    ) -> SecureFsResult<()> {
        if identity.device != root_device {
            return Err(unsafe_object(path, "census leaf crosses the root mount"));
        }
        if identity.links != 1 {
            return Err(unsafe_object(
                path,
                "census leaf must have exactly one filesystem link",
            ));
        }
        Ok(())
    }

    fn ensure_no_semantic_metadata(file: &File, path: &Path) -> SecureFsResult<()> {
        #[cfg(target_os = "linux")]
        let length = unsafe { flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) };
        #[cfg(target_os = "macos")]
        let length = unsafe { flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0, 0) };
        if length < 0 {
            return Err(SecureFsError::Io {
                operation: "enumerate semantic metadata without following paths",
                path: path.to_path_buf(),
                source: io::Error::last_os_error(),
            });
        }
        if length == 0 {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            let mut names = vec![0_u8; length as usize];
            let observed = unsafe {
                flistxattr(
                    file.as_raw_fd(),
                    names.as_mut_ptr().cast::<c_char>(),
                    names.len(),
                    0,
                )
            };
            if observed != length {
                return Err(SecureFsError::ChangedDuringRead {
                    path: path.to_path_buf(),
                });
            }
            let only_kernel_provenance = names
                .split(|byte| *byte == 0)
                .filter(|name| !name.is_empty())
                .all(|name| name == b"com.apple.provenance");
            if only_kernel_provenance {
                return Ok(());
            }
        }
        Err(unsafe_object(
            path,
            "semantic metadata streams are unsupported by this census profile",
        ))
    }

    #[cfg(target_os = "linux")]
    fn mount_identity(file: &File, path: &Path) -> SecureFsResult<[u8; 32]> {
        let fdinfo_path = format!("/proc/self/fdinfo/{}", file.as_raw_fd());
        let fdinfo = std::fs::read_to_string(&fdinfo_path).map_err(|source| SecureFsError::Io {
            operation: "read descriptor mount identity",
            path: path.to_path_buf(),
            source,
        })?;
        let mount_id = fdinfo
            .lines()
            .find_map(|line| line.strip_prefix("mnt_id:\t"))
            .ok_or_else(|| unsafe_object(path, "descriptor mount identity is unavailable"))?;
        if !mount_id.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(unsafe_object(
                path,
                "descriptor mount identity is malformed",
            ));
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.foundation.mount-identity.linux.v1\0");
        hasher.update(mount_id.as_bytes());
        Ok(hasher.finalize().into())
    }

    #[cfg(target_os = "macos")]
    fn mount_identity(file: &File, path: &Path) -> SecureFsResult<[u8; 32]> {
        let mut status = std::mem::MaybeUninit::<DarwinStatFs>::zeroed();
        if unsafe { fstatfs(file.as_raw_fd(), status.as_mut_ptr()) } != 0 {
            return Err(SecureFsError::Io {
                operation: "read descriptor mount identity",
                path: path.to_path_buf(),
                source: io::Error::last_os_error(),
            });
        }
        let status = unsafe { status.assume_init() };
        let mount_name = unsafe { CStr::from_ptr(status.f_mntonname.as_ptr()) }.to_bytes();
        if mount_name.is_empty() {
            return Err(unsafe_object(path, "descriptor mount point is unavailable"));
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.foundation.mount-identity.macos.v1\0");
        for part in status.f_fsid {
            hasher.update(part.to_be_bytes());
        }
        hasher.update((mount_name.len() as u64).to_be_bytes());
        hasher.update(mount_name);
        Ok(hasher.finalize().into())
    }

    fn census_object_identity(identity: FileIdentity) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.foundation.descriptor-object-binding.v1\0");
        hasher.update(identity.device.to_be_bytes());
        hasher.update(identity.inode.to_be_bytes());
        hasher.update(identity.len.to_be_bytes());
        hasher.update(identity.mtime.0.to_be_bytes());
        hasher.update(identity.mtime.1.to_be_bytes());
        hasher.update(identity.ctime.0.to_be_bytes());
        hasher.update(identity.ctime.1.to_be_bytes());
        hasher.update(identity.links.to_be_bytes());
        hasher.update(identity.owner.to_be_bytes());
        hasher.update(identity.mode.to_be_bytes());
        hasher.finalize().into()
    }

    fn read_link_at(directory: RawFd, name: &CStr, path: &Path) -> SecureFsResult<Option<Vec<u8>>> {
        let mut bytes = vec![0u8; MAX_PATH_BYTES + 1];
        let length = unsafe {
            readlinkat(
                directory,
                name.as_ptr(),
                bytes.as_mut_ptr().cast::<c_char>(),
                bytes.len(),
            )
        };
        if length == -1 {
            let source = io::Error::last_os_error();
            if source.raw_os_error() == Some(22) {
                return Ok(None);
            }
            return Err(SecureFsError::Io {
                operation: "read symbolic link without following it",
                path: path.to_path_buf(),
                source,
            });
        }
        let length =
            usize::try_from(length).map_err(|_| unsafe_object(path, "invalid link size"))?;
        if length == 0 || length > MAX_PATH_BYTES {
            return Err(unsafe_object(
                path,
                "symbolic-link target is not lossless and bounded",
            ));
        }
        bytes.truncate(length);
        Ok(Some(bytes))
    }

    fn open_symlink_at(directory: RawFd, name: &CStr, path: &Path) -> SecureFsResult<File> {
        #[cfg(target_os = "linux")]
        const OPEN_LINK: c_int = 0o10000000 | flags::O_NOFOLLOW;
        #[cfg(target_os = "macos")]
        const OPEN_LINK: c_int = 0x20_0000;
        let descriptor = unsafe { openat(directory, name.as_ptr(), OPEN_LINK | flags::O_CLOEXEC) };
        descriptor_to_file(
            descriptor,
            "open symbolic-link identity without following it",
            path,
        )
    }

    #[cfg(target_os = "linux")]
    unsafe fn directory_entry_name(record: &DirectoryEntryRecord) -> SecureFsResult<&[u8]> {
        let bytes = unsafe {
            std::slice::from_raw_parts(record.d_name.as_ptr().cast::<u8>(), record.d_name.len())
        };
        let length = bytes.iter().position(|byte| *byte == 0).ok_or_else(|| {
            SecureFsError::UnsafeObject {
                path: PathBuf::new(),
                reason: "directory entry name is not nul terminated",
            }
        })?;
        Ok(&bytes[..length])
    }

    #[cfg(target_os = "macos")]
    unsafe fn directory_entry_name(record: &DirectoryEntryRecord) -> SecureFsResult<&[u8]> {
        let length = usize::from(record.d_namlen);
        if length >= record.d_name.len() {
            return Err(SecureFsError::UnsafeObject {
                path: PathBuf::new(),
                reason: "directory entry name exceeds the platform record",
            });
        }
        Ok(unsafe { std::slice::from_raw_parts(record.d_name.as_ptr().cast::<u8>(), length) })
    }

    #[cfg(target_os = "linux")]
    unsafe fn errno_location() -> *mut c_int {
        unsafe { __errno_location() }
    }

    #[cfg(target_os = "macos")]
    unsafe fn errno_location() -> *mut c_int {
        unsafe { __error() }
    }

    fn is_secure_temp_name(name: &[u8]) -> bool {
        let Some(body) = name
            .strip_prefix(b".maestro-secure-")
            .and_then(|value| value.strip_suffix(b".tmp"))
        else {
            return false;
        };
        let mut parts = body.split(|byte| *byte == b'.');
        let valid_number = |part: &[u8]| !part.is_empty() && part.iter().all(u8::is_ascii_digit);
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(process), Some(counter), None)
                if valid_number(process) && valid_number(counter)
        )
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

    fn update_object_identity(hasher: &mut Sha256, identity: ObjectIdentity) {
        hasher.update(identity.device.to_be_bytes());
        hasher.update(identity.inode.to_be_bytes());
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
        let mut descriptor_chain = Vec::new();
        for component in anchored.components() {
            let Component::Normal(component) = component else {
                continue;
            };
            let name = c_string(component, &anchored)?;
            let next = if create {
                ensure_root_directory_at(&directory, &name, &anchored)?
            } else {
                open_directory_at_unchecked(directory.as_raw_fd(), &name, &anchored)?
            };
            descriptor_chain.push(RetainedDescriptorEdgeV1 {
                parent: directory,
                child_name: name,
            });
            directory = next;
        }
        validate_directory(&directory, &anchored)?;
        let namespace_anchor = open_directory_at_unchecked(directory.as_raw_fd(), c".", &anchored)?;
        Ok(SecureRoot {
            directory,
            namespace_anchor,
            descriptor_chain,
            path: anchored,
        })
    }

    fn clone_descriptor(file: &File, path: &Path) -> SecureFsResult<File> {
        file.try_clone().map_err(|source| SecureFsError::Io {
            operation: "retain descriptor-anchored ancestor",
            path: path.to_path_buf(),
            source,
        })
    }

    fn clone_descriptor_chain(
        chain: &[RetainedDescriptorEdgeV1],
        path: &Path,
    ) -> SecureFsResult<Vec<RetainedDescriptorEdgeV1>> {
        chain
            .iter()
            .map(|edge| {
                Ok(RetainedDescriptorEdgeV1 {
                    parent: clone_descriptor(&edge.parent, path)?,
                    child_name: edge.child_name.clone(),
                })
            })
            .collect()
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

    pub(super) fn removal_quarantine_name_for_digest(digest: &[u8; 32]) -> CString {
        let mut name = String::from(".maestro-remove-");
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut name, "{byte:02x}")
                .expect("invariant: writing a digest into a String cannot fail");
        }
        name.push_str(".pending");
        CString::new(name).expect("invariant: generated quarantine name has no nul byte")
    }

    pub(super) fn removal_debt_bytes(leaf: &CStr, digest: &[u8; 32]) -> Vec<u8> {
        let mut bytes = b"maestro.secure-removal-debt.v1\0".to_vec();
        bytes.extend_from_slice(leaf.to_bytes());
        bytes.push(0);
        bytes.extend_from_slice(digest);
        bytes
    }

    pub(super) fn removal_debt_name_for_digest(leaf: &CStr, digest: &[u8; 32]) -> CString {
        let marker_digest: [u8; 32] = Sha256::digest(removal_debt_bytes(leaf, digest)).into();
        let mut name = String::from(".maestro-remove-");
        for byte in marker_digest {
            use std::fmt::Write as _;
            write!(&mut name, "{byte:02x}")
                .expect("invariant: writing a digest into a String cannot fail");
        }
        name.push_str(".debt");
        CString::new(name).expect("invariant: generated removal debt name has no nul byte")
    }

    pub(super) fn removal_sentinel_name_for_digest(leaf: &CStr, digest: &[u8; 32]) -> CString {
        removal_state_name(leaf, digest, ".sentinel")
    }

    pub(super) fn removal_resolution_name_for_digest(leaf: &CStr, digest: &[u8; 32]) -> CString {
        removal_state_name(leaf, digest, ".resolved")
    }

    fn removal_state_name(leaf: &CStr, digest: &[u8; 32], suffix: &str) -> CString {
        let marker_digest: [u8; 32] = Sha256::digest(removal_debt_bytes(leaf, digest)).into();
        let mut name = String::from(".maestro-remove-");
        for byte in marker_digest {
            use std::fmt::Write as _;
            write!(&mut name, "{byte:02x}")
                .expect("invariant: writing a digest into a String cannot fail");
        }
        name.push_str(suffix);
        CString::new(name).expect("invariant: generated removal state name has no nul byte")
    }

    pub(super) fn removal_resolution_bytes(leaf: &CStr, digest: &[u8; 32]) -> Vec<u8> {
        let mut bytes = b"maestro.secure-removal-resolution.v1\0".to_vec();
        bytes.extend_from_slice(leaf.to_bytes());
        bytes.push(0);
        bytes.extend_from_slice(digest);
        bytes
    }

    struct RemovalState {
        quarantine: CString,
        debt: CString,
        debt_bytes: Vec<u8>,
        sentinel: CString,
        resolution: CString,
        resolution_bytes: Vec<u8>,
    }

    impl RemovalState {
        fn new(leaf: &CStr, digest: &[u8; 32], quarantine: CString) -> Self {
            Self {
                quarantine,
                debt: removal_debt_name_for_digest(leaf, digest),
                debt_bytes: removal_debt_bytes(leaf, digest),
                sentinel: removal_sentinel_name_for_digest(leaf, digest),
                resolution: removal_resolution_name_for_digest(leaf, digest),
                resolution_bytes: removal_resolution_bytes(leaf, digest),
            }
        }
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
        state: &RemovalState,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, &state.quarantine, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                return finish_sentinel_removal(parent, state, expected, path);
            }
            Err(error) => return Err(error),
        };
        validate_regular_file_allow_links(&candidate, path)?;
        if read_stable(&mut candidate, path)? != expected {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        finish_open_quarantined_removal(parent, state, &candidate, path)
    }

    fn finish_quarantined_removal_by_digest(
        parent: &File,
        state: &RemovalState,
        expected_digest: &[u8; 32],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, &state.quarantine, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                return finish_sentinel_removal_by_digest(parent, state, expected_digest, path);
            }
            Err(error) => return Err(error),
        };
        validate_regular_file_allow_links(&candidate, path)?;
        let bytes = read_stable(&mut candidate, path)?;
        let observed: [u8; 32] = Sha256::digest(&bytes).into();
        if &observed != expected_digest {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        finish_open_quarantined_removal(parent, state, &candidate, path)
    }

    fn finish_open_quarantined_removal(
        parent: &File,
        state: &RemovalState,
        candidate: &File,
        path: &Path,
    ) -> SecureFsResult<bool> {
        ensure_removal_debt_marker(parent, &state.debt, &state.debt_bytes, path)?;
        ensure_removal_sentinel(parent, &state.quarantine, &state.sentinel, candidate, path)?;
        run_before_removal_unlink_test_hook(&removal_sibling_path(path, &state.quarantine));
        unlink_at(
            parent.as_raw_fd(),
            &state.quarantine,
            0,
            path,
            "remove quarantined file",
        )?;
        sync_directory(parent, path)?;
        if metadata(candidate, path)?.nlink() != 1 {
            return Err(SecureFsError::UnsafeObject {
                path: path.to_path_buf(),
                reason: "removed file still has a hard-link alias",
            });
        }
        finish_open_sentinel_removal(parent, state, candidate, path)?;
        Ok(true)
    }

    fn finish_sentinel_removal(
        parent: &File,
        state: &RemovalState,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, &state.sentinel, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                return finish_resolved_removal_without_payload(
                    parent,
                    &state.debt,
                    &state.debt_bytes,
                    &state.resolution,
                    &state.resolution_bytes,
                    path,
                );
            }
            Err(error) => return Err(error),
        };
        validate_regular_file_allow_links(&candidate, path)?;
        if read_stable(&mut candidate, path)? != expected {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        finish_open_sentinel_removal(parent, state, &candidate, path)?;
        Ok(true)
    }

    fn finish_sentinel_removal_by_digest(
        parent: &File,
        state: &RemovalState,
        expected_digest: &[u8; 32],
        path: &Path,
    ) -> SecureFsResult<bool> {
        let mut candidate = match open_regular_file_at(parent, &state.sentinel, path) {
            Ok(candidate) => candidate,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                return finish_resolved_removal_without_payload(
                    parent,
                    &state.debt,
                    &state.debt_bytes,
                    &state.resolution,
                    &state.resolution_bytes,
                    path,
                );
            }
            Err(error) => return Err(error),
        };
        validate_regular_file_allow_links(&candidate, path)?;
        let observed: [u8; 32] = Sha256::digest(read_stable(&mut candidate, path)?).into();
        if &observed != expected_digest {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        finish_open_sentinel_removal(parent, state, &candidate, path)?;
        Ok(true)
    }

    fn finish_open_sentinel_removal(
        parent: &File,
        state: &RemovalState,
        candidate: &File,
        path: &Path,
    ) -> SecureFsResult<()> {
        ensure_removal_debt_marker(parent, &state.debt, &state.debt_bytes, path)?;
        if metadata(candidate, path)?.nlink() != 1 {
            return Err(SecureFsError::UnsafeObject {
                path: path.to_path_buf(),
                reason: "removed file still has a hard-link alias",
            });
        }
        run_after_removal_sentinel_check_test_hook(&removal_sibling_path(path, &state.sentinel));
        unlink_at(
            parent.as_raw_fd(),
            &state.sentinel,
            0,
            path,
            "remove exclusive file-removal sentinel",
        )?;
        if metadata(candidate, path)?.nlink() != 0 {
            return Err(SecureFsError::UnsafeObject {
                path: path.to_path_buf(),
                reason: "removed file still has a hard-link alias",
            });
        }
        sync_directory(parent, path)?;
        ensure_exact_marker(parent, &state.resolution, &state.resolution_bytes, path)?;
        clear_resolved_removal_markers(
            parent,
            &state.debt,
            &state.debt_bytes,
            &state.resolution,
            &state.resolution_bytes,
            path,
        )
    }

    fn ensure_removal_sentinel(
        parent: &File,
        quarantine: &CStr,
        sentinel: &CStr,
        candidate: &File,
        path: &Path,
    ) -> SecureFsResult<()> {
        let result = unsafe {
            linkat(
                parent.as_raw_fd(),
                quarantine.as_ptr(),
                parent.as_raw_fd(),
                sentinel.as_ptr(),
                0,
            )
        };
        if result == 0 {
            sync_directory(parent, path)?;
        } else {
            let source = io::Error::last_os_error();
            if source.kind() != io::ErrorKind::AlreadyExists {
                return Err(SecureFsError::Io {
                    operation: "create file-removal sentinel",
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        let existing = open_regular_file_at(parent, sentinel, path)?;
        validate_regular_file_allow_links(&existing, path)?;
        if ObjectIdentity::from(&metadata(&existing, path)?)
            != ObjectIdentity::from(&metadata(candidate, path)?)
        {
            return Err(SecureFsError::ChangedDuringRead {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn ensure_removal_debt_marker(
        parent: &File,
        debt: &CStr,
        debt_bytes: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        ensure_exact_marker(parent, debt, debt_bytes, path)
    }

    fn ensure_exact_marker(
        parent: &File,
        name: &CStr,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        let temp = create_temp_file(parent, expected, path)?;
        match rename_no_replace(parent.as_raw_fd(), &temp.name, name, path) {
            Ok(()) => {
                verify_published_file(parent, name, &temp.file, expected, path)?;
                sync_directory(parent, path)
            }
            Err(error) if error.io_kind(io::ErrorKind::AlreadyExists) => {
                cleanup_temp(parent, &temp.name, path)?;
                let mut existing = open_regular_file_at(parent, name, path)?;
                validate_regular_file(&existing, path)?;
                if read_stable(&mut existing, path)? != expected {
                    return Err(SecureFsError::ContentMismatch {
                        path: path.to_path_buf(),
                    });
                }
                Ok(())
            }
            Err(error) => {
                let _ = cleanup_temp(parent, &temp.name, path);
                Err(error)
            }
        }
    }

    fn finish_resolved_removal_without_payload(
        parent: &File,
        debt: &CStr,
        debt_bytes: &[u8],
        resolution: &CStr,
        resolution_bytes: &[u8],
        path: &Path,
    ) -> SecureFsResult<bool> {
        match open_regular_file_at(parent, resolution, path) {
            Ok(mut marker) => {
                validate_regular_file(&marker, path)?;
                if read_stable(&mut marker, path)? != resolution_bytes {
                    return Err(SecureFsError::ContentMismatch {
                        path: path.to_path_buf(),
                    });
                }
                clear_resolved_removal_markers(
                    parent,
                    debt,
                    debt_bytes,
                    resolution,
                    resolution_bytes,
                    path,
                )?;
                Ok(false)
            }
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {
                verify_removal_debt_absent(parent, debt, debt_bytes, path)?;
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    fn clear_resolved_removal_markers(
        parent: &File,
        debt: &CStr,
        debt_bytes: &[u8],
        resolution: &CStr,
        resolution_bytes: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        remove_exact_marker_if_present(parent, debt, debt_bytes, path)?;
        sync_directory(parent, path)?;
        remove_exact_marker_if_present(parent, resolution, resolution_bytes, path)?;
        sync_directory(parent, path)
    }

    fn remove_exact_marker_if_present(
        parent: &File,
        name: &CStr,
        expected: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        let mut marker = match open_regular_file_at(parent, name, path) {
            Ok(marker) => marker,
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => return Ok(()),
            Err(error) => return Err(error),
        };
        validate_regular_file(&marker, path)?;
        if read_stable(&mut marker, path)? != expected {
            return Err(SecureFsError::ContentMismatch {
                path: path.to_path_buf(),
            });
        }
        unlink_at(
            parent.as_raw_fd(),
            name,
            0,
            path,
            "clear resolved removal marker",
        )
    }

    fn verify_removal_state_absent(
        parent: &File,
        state: &RemovalState,
        path: &Path,
    ) -> SecureFsResult<()> {
        match open_regular_file_at(parent, &state.sentinel, path) {
            Ok(candidate) => {
                validate_regular_file_allow_links(&candidate, path)?;
                return Err(SecureFsError::UnsafeObject {
                    path: path.to_path_buf(),
                    reason: "file removal has an unresolved payload sentinel",
                });
            }
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => {}
            Err(error) => return Err(error),
        }
        match open_regular_file_at(parent, &state.resolution, path) {
            Ok(mut marker) => {
                validate_regular_file(&marker, path)?;
                if read_stable(&mut marker, path)? != state.resolution_bytes {
                    return Err(SecureFsError::ContentMismatch {
                        path: path.to_path_buf(),
                    });
                }
                Err(SecureFsError::UnsafeObject {
                    path: path.to_path_buf(),
                    reason: "file removal recovery has not cleared its resolution marker",
                })
            }
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn verify_removal_debt_absent(
        parent: &File,
        debt: &CStr,
        debt_bytes: &[u8],
        path: &Path,
    ) -> SecureFsResult<()> {
        match open_regular_file_at(parent, debt, path) {
            Ok(mut marker) => {
                validate_regular_file(&marker, path)?;
                if read_stable(&mut marker, path)? != debt_bytes {
                    return Err(SecureFsError::ContentMismatch {
                        path: path.to_path_buf(),
                    });
                }
                Err(SecureFsError::UnsafeObject {
                    path: path.to_path_buf(),
                    reason: "file removal has unresolved hard-link or crash debt",
                })
            }
            Err(error) if error.io_kind(io::ErrorKind::NotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn removal_sibling_path(path: &Path, name: &CStr) -> PathBuf {
        path.parent()
            .expect("invariant: a bounded file path has a parent")
            .join(OsStr::from_bytes(name.to_bytes()))
    }

    #[cfg(test)]
    fn run_before_removal_unlink_test_hook(quarantine_path: &Path) {
        BEFORE_REMOVAL_UNLINK_TEST_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow_mut().take() {
                hook(quarantine_path);
            }
        });
    }

    #[cfg(not(test))]
    fn run_before_removal_unlink_test_hook(_quarantine_path: &Path) {}

    #[cfg(test)]
    fn run_after_removal_sentinel_check_test_hook(sentinel_path: &Path) {
        AFTER_REMOVAL_SENTINEL_CHECK_TEST_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow_mut().take() {
                hook(sentinel_path);
            }
        });
    }

    #[cfg(not(test))]
    fn run_after_removal_sentinel_check_test_hook(_sentinel_path: &Path) {}

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

    fn validate_regular_file_allow_links(file: &File, path: &Path) -> SecureFsResult<()> {
        let metadata = metadata(file, path)?;
        if !metadata.is_file() {
            return Err(unsafe_object(path, "expected a regular file"));
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
    #[cfg(test)]
    use super::DescriptorCensusMutationFenceV1;
    use super::{
        CreateIfAbsent, DescriptorAnchoredCensusV1, DescriptorCensusLimitsV1, RegularFileBinding,
        SecureDirectoryEntry, SecureFsError, SecureFsResult,
    };
    use std::path::Path;

    #[derive(Debug)]
    pub struct SecureRoot;

    pub struct AdmittedDescriptorCensusRootV1<'root> {
        _root: std::marker::PhantomData<&'root SecureRoot>,
    }

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
        pub(crate) fn read_dir_entries(&self) -> SecureFsResult<Vec<SecureDirectoryEntry>> {
            unsupported()
        }
        #[cfg(test)]
        pub(crate) fn descriptor_anchored_census<F: DescriptorCensusMutationFenceV1>(
            &self,
            _limits: DescriptorCensusLimitsV1,
            _fence: &mut F,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
            unsupported()
        }
        pub(in crate::foundation::core) fn admit_descriptor_census_root(
            &self,
        ) -> SecureFsResult<AdmittedDescriptorCensusRootV1<'_>> {
            unsupported()
        }
        pub(in crate::foundation::core) fn census_admitted_descriptor_root(
            _admitted: AdmittedDescriptorCensusRootV1<'_>,
            _limits: DescriptorCensusLimitsV1,
        ) -> SecureFsResult<DescriptorAnchoredCensusV1> {
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
        pub(crate) fn verify_file_removal_resolved(
            &self,
            _path: impl AsRef<Path>,
            _expected_digest: &[u8; 32],
        ) -> SecureFsResult<()> {
            unsupported()
        }
        pub(crate) fn remove_crash_residual_temps_by_digest(
            &self,
            _directory: impl AsRef<Path>,
            _expected_digest: &[u8; 32],
        ) -> SecureFsResult<usize> {
            unsupported()
        }
        pub(crate) fn verify_no_crash_residual_temp_by_digest(
            &self,
            _directory: impl AsRef<Path>,
            _expected_digest: &[u8; 32],
        ) -> SecureFsResult<()> {
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

pub use platform::{AdmittedDescriptorCensusRootV1, SecureRoot};

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::cell::Cell;
    use std::ffi::{CString, OsStr};
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::platform::{
        removal_debt_bytes, removal_debt_name_for_digest, removal_quarantine_name_for_digest,
        removal_resolution_bytes, removal_resolution_name_for_digest,
        removal_sentinel_name_for_digest,
    };
    use super::{
        CreateIfAbsent, DescriptorCensusLimitsV1, DescriptorCensusMutationFenceFactsV1,
        DescriptorCensusMutationFenceV1, DescriptorCensusMutationLeasePortV1,
        DescriptorCensusObjectKindV1, DescriptorCensusRootBindingV1, MAX_COMPONENT_BYTES,
        MAX_COMPONENTS, MAX_FILE_BYTES, MAX_PATH_BYTES, SecureFsError, SecureRoot,
        descriptor_census_sealed,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestCensusFence {
        epoch: Cell<u64>,
    }

    impl TestCensusFence {
        const fn new() -> Self {
            Self {
                epoch: Cell::new(1),
            }
        }
    }

    trait TestCensusFenceStateV1 {
        fn current_facts(&self) -> Option<DescriptorCensusMutationFenceFactsV1>;
    }

    impl TestCensusFenceStateV1 for TestCensusFence {
        fn current_facts(&self) -> Option<DescriptorCensusMutationFenceFactsV1> {
            DescriptorCensusMutationFenceFactsV1::from_live_owner(
                [1; 32],
                [2; 32],
                self.epoch.get(),
            )
        }
    }

    struct TestCensusLeaseV1<'lease, F> {
        provider: &'lease mut F,
        initial: DescriptorCensusMutationFenceFactsV1,
        root_binding: DescriptorCensusRootBindingV1,
    }

    impl<F> descriptor_census_sealed::LeaseSealed for TestCensusLeaseV1<'_, F> {}

    impl<F: TestCensusFenceStateV1> DescriptorCensusMutationLeasePortV1 for TestCensusLeaseV1<'_, F> {
        fn facts(&self) -> DescriptorCensusMutationFenceFactsV1 {
            self.initial
        }

        fn root_binding(&self) -> DescriptorCensusRootBindingV1 {
            self.root_binding
        }

        fn consume_final_recheck(self) -> bool {
            self.provider.current_facts() == Some(self.initial)
        }
    }

    impl descriptor_census_sealed::FenceSealed for TestCensusFence {}

    impl DescriptorCensusMutationFenceV1 for TestCensusFence {
        type Lease<'lease> = TestCensusLeaseV1<'lease, Self>;

        fn acquire_exclusive(
            &mut self,
            root_binding: DescriptorCensusRootBindingV1,
        ) -> Option<Self::Lease<'_>> {
            let initial = self.current_facts()?;
            Some(TestCensusLeaseV1 {
                provider: self,
                initial,
                root_binding,
            })
        }
    }

    struct TurningCensusFence {
        calls: Cell<u64>,
    }

    impl TestCensusFenceStateV1 for TurningCensusFence {
        fn current_facts(&self) -> Option<DescriptorCensusMutationFenceFactsV1> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            DescriptorCensusMutationFenceFactsV1::from_live_owner([1; 32], [2; 32], call)
        }
    }

    impl descriptor_census_sealed::FenceSealed for TurningCensusFence {}

    impl DescriptorCensusMutationFenceV1 for TurningCensusFence {
        type Lease<'lease> = TestCensusLeaseV1<'lease, Self>;

        fn acquire_exclusive(
            &mut self,
            root_binding: DescriptorCensusRootBindingV1,
        ) -> Option<Self::Lease<'_>> {
            let initial = self.current_facts()?;
            Some(TestCensusLeaseV1 {
                provider: self,
                initial,
                root_binding,
            })
        }
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            Self::under(fs::canonicalize(std::env::temp_dir()).expect("canonical temp root"))
        }

        #[cfg(target_os = "macos")]
        fn for_descriptor_census() -> Self {
            Self::under(
                fs::canonicalize("/private/tmp").expect("canonical descriptor census temp root"),
            )
        }

        #[cfg(not(target_os = "macos"))]
        fn for_descriptor_census() -> Self {
            Self::new()
        }

        fn under(temp_root: PathBuf) -> Self {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
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
    fn digest_addressed_removal_recovers_after_payload_unlink_and_marker_crashes() {
        use sha2::{Digest, Sha256};

        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let bytes = b"object bytes";
        root.create_file_if_absent("object", bytes)
            .expect("create object");
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let leaf = CString::new("object").expect("leaf");
        let quarantine = removal_quarantine_name_for_digest(&digest);
        let debt = removal_debt_name_for_digest(&leaf, &digest);
        let sentinel = removal_sentinel_name_for_digest(&leaf, &digest);
        let resolution = removal_resolution_name_for_digest(&leaf, &digest);
        fs::rename(
            temp.0.join("object"),
            temp.0.join(OsStr::from_bytes(quarantine.to_bytes())),
        )
        .expect("quarantine payload");
        fs::write(
            temp.0.join(OsStr::from_bytes(debt.to_bytes())),
            removal_debt_bytes(&leaf, &digest),
        )
        .expect("persist debt");
        fs::hard_link(
            temp.0.join(OsStr::from_bytes(quarantine.to_bytes())),
            temp.0.join(OsStr::from_bytes(sentinel.to_bytes())),
        )
        .expect("persist sentinel");
        fs::remove_file(temp.0.join(OsStr::from_bytes(quarantine.to_bytes())))
            .expect("simulate crash after payload unlink");
        drop(root);

        let reopened = SecureRoot::open(&temp.0).expect("reopen root");
        assert!(
            reopened
                .finish_file_removal_if_digest_matches("object", &digest)
                .expect("recover sentinel removal")
        );
        reopened
            .verify_file_removal_resolved("object", &digest)
            .expect("sentinel recovery resolved");

        fs::write(
            temp.0.join(OsStr::from_bytes(debt.to_bytes())),
            removal_debt_bytes(&leaf, &digest),
        )
        .expect("persist debt before final marker cleanup");
        fs::write(
            temp.0.join(OsStr::from_bytes(resolution.to_bytes())),
            removal_resolution_bytes(&leaf, &digest),
        )
        .expect("persist resolved marker");
        assert!(
            !reopened
                .finish_file_removal_if_digest_matches("object", &digest)
                .expect("recover final marker cleanup")
        );
        reopened
            .verify_file_removal_resolved("object", &digest)
            .expect("marker cleanup resolved");
    }

    #[test]
    fn crash_residual_temp_blocks_absence_until_digest_bound_cleanup() {
        use sha2::{Digest, Sha256};

        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("objects").expect("create objects");
        let bytes = b"secret crash residual";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        fs::write(temp.0.join("objects/.maestro-secure-999999.17.tmp"), bytes)
            .expect("simulate a complete pre-rename crash residual");

        assert!(matches!(
            root.verify_file_removal_resolved("objects/object", &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "crash-residual temporary file still carries removed bytes",
                ..
            })
        ));
        assert_eq!(
            root.remove_crash_residual_temps_by_digest("objects", &digest)
                .expect("remove residual through digest-bound protocol"),
            1
        );
        root.verify_file_removal_resolved("objects/object", &digest)
            .expect("physical absence includes temporary carrier census");

        fs::write(temp.0.join("objects/.maestro-secure-999999.18.tmp"), bytes)
            .expect("create residual before directory replacement race");
        let opened_objects = root
            .open_dir("objects")
            .expect("open exact objects directory");
        fs::rename(temp.0.join("objects"), temp.0.join("objects-displaced"))
            .expect("displace opened objects directory");
        fs::create_dir(temp.0.join("objects")).expect("install empty path replacement");
        assert!(matches!(
            opened_objects.verify_no_crash_residual_temp_by_digest(PathBuf::from("."), &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "crash-residual temporary file still carries removed bytes",
                ..
            })
        ));
    }

    #[test]
    fn hard_link_race_leaves_durable_removal_debt_across_restart() {
        use sha2::{Digest, Sha256};

        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let bytes = b"secret object bytes";
        root.create_file_if_absent("object", bytes)
            .expect("create object");
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let alias = temp.0.join("escaped-alias");
        let alias_for_hook = alias.clone();
        SecureRoot::install_before_removal_unlink_test_hook(move |quarantine| {
            fs::hard_link(quarantine, &alias_for_hook).expect("create the racing hard-link alias");
        });

        assert!(matches!(
            root.remove_file_if_matches("object", bytes),
            Err(SecureFsError::UnsafeObject {
                reason: "removed file still has a hard-link alias",
                ..
            })
        ));
        assert!(alias.is_file());
        drop(root);

        let reopened = SecureRoot::open(&temp.0).expect("reopen root");
        assert!(matches!(
            reopened.verify_file_removal_resolved("object", &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "file removal has unresolved hard-link or crash debt",
                ..
            })
        ));
        assert!(matches!(
            reopened.finish_file_removal_if_digest_matches("object", &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "removed file still has a hard-link alias",
                ..
            })
        ));
    }

    #[test]
    fn hard_link_after_sentinel_check_never_publishes_resolution() {
        use sha2::{Digest, Sha256};

        let temp = TestDir::new();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        let bytes = b"secret object bytes after sentinel check";
        root.create_file_if_absent("object", bytes)
            .expect("create object");
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let alias = temp.0.join("escaped-after-check-alias");
        let alias_for_hook = alias.clone();
        SecureRoot::install_after_removal_sentinel_check_test_hook(move |sentinel| {
            fs::hard_link(sentinel, &alias_for_hook)
                .expect("create hard-link after the first sentinel link-count check");
        });

        assert!(matches!(
            root.remove_file_if_matches("object", bytes),
            Err(SecureFsError::UnsafeObject {
                reason: "removed file still has a hard-link alias",
                ..
            })
        ));
        assert!(alias.is_file());
        drop(root);

        let reopened = SecureRoot::open(&temp.0).expect("reopen root");
        assert!(matches!(
            reopened.finish_file_removal_if_digest_matches("object", &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "file removal has unresolved hard-link or crash debt",
                ..
            })
        ));
        assert!(matches!(
            reopened.verify_file_removal_resolved("object", &digest),
            Err(SecureFsError::UnsafeObject {
                reason: "file removal has unresolved hard-link or crash debt",
                ..
            })
        ));
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
            Err(SecureFsError::CensusRefused)
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
    fn descriptor_census_binds_regular_files_and_symlinks_without_following() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("nested")
            .expect("create nested directory");
        root.create_file_if_absent("nested/object", b"immutable")
            .expect("create object");
        symlink("nested/object", temp.0.join("object-link")).expect("create symlink");

        let census = root
            .descriptor_anchored_census(
                DescriptorCensusLimitsV1::bounded(
                    MAX_COMPONENTS,
                    100_000,
                    MAX_FILE_BYTES,
                    MAX_FILE_BYTES * 16,
                    MAX_PATH_BYTES,
                    MAX_COMPONENT_BYTES,
                )
                .expect("bounded census limits"),
                &mut TestCensusFence::new(),
            )
            .expect("build census");
        assert_ne!(census.identity(), [0; 32]);
        assert_eq!(census.rows().len(), 2);
        assert_eq!(census.rows()[0].relative_name(), b"nested/object");
        assert_eq!(
            census.rows()[0].kind(),
            DescriptorCensusObjectKindV1::RegularFile
        );
        assert_ne!(census.rows()[0].object_identity(), [0; 32]);
        assert_ne!(census.rows()[0].content_identity(), [0; 32]);
        assert_eq!(census.rows()[0].logical_byte_length(), 9);
        assert_eq!(census.rows()[1].relative_name(), b"object-link");
        assert_eq!(
            census.rows()[1].kind(),
            DescriptorCensusObjectKindV1::SymbolicLink
        );
    }

    #[test]
    fn platform_descriptor_census_holds_the_foundation_namespace_fence() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("object", b"immutable")
            .expect("create object");

        let census = crate::foundation::core::descriptor_census_platform::admit_and_census(
            &root,
            DescriptorCensusLimitsV1::bounded_default(),
        )
        .expect("platform census");
        assert_eq!(census.rows().len(), 1);
        assert_eq!(census.rows()[0].relative_name(), b"object");
    }

    #[test]
    fn platform_descriptor_census_refuses_fence_loss_and_external_mutation() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("object", b"immutable")
            .expect("create object");
        SecureRoot::force_platform_census_fence_loss_for_test();
        assert!(matches!(
            crate::foundation::core::descriptor_census_platform::admit_and_census(
                &root,
                DescriptorCensusLimitsV1::bounded_default()
            ),
            Err(SecureFsError::CensusRefused)
        ));

        SecureRoot::install_after_first_census_pass_test_hook(|root| {
            fs::write(root.join("object"), b"substitute").expect("mutate outside writer fence");
        });
        assert!(matches!(
            crate::foundation::core::descriptor_census_platform::admit_and_census(
                &root,
                DescriptorCensusLimitsV1::bounded_default()
            ),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn descriptor_census_refuses_every_hard_linked_leaf() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("object", b"immutable")
            .expect("create object");
        fs::hard_link(temp.0.join("object"), temp.0.join("alias")).expect("create hard link");

        assert!(matches!(
            root.descriptor_anchored_census(
                DescriptorCensusLimitsV1::bounded_default(),
                &mut TestCensusFence::new()
            ),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn descriptor_census_refuses_mutation_fence_turnover() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_file_if_absent("object", b"immutable")
            .expect("create object");
        let mut fence = TurningCensusFence {
            calls: Cell::new(0),
        };
        assert!(matches!(
            root.descriptor_anchored_census(
                DescriptorCensusLimitsV1::bounded_default(),
                &mut fence
            ),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn descriptor_census_charges_empty_directories_before_buffer_growth() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("first")
            .expect("create first directory");
        root.create_dir_all("second")
            .expect("create second directory");

        let limits = DescriptorCensusLimitsV1::bounded(
            MAX_COMPONENTS,
            1,
            MAX_FILE_BYTES,
            MAX_FILE_BYTES,
            MAX_PATH_BYTES,
            MAX_COMPONENT_BYTES,
        )
        .expect("bounded census limits");
        assert!(matches!(
            root.descriptor_anchored_census(limits, &mut TestCensusFence::new()),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn descriptor_census_applies_depth_limit_to_leaves_and_hides_names() {
        let temp = TestDir::for_descriptor_census();
        let root = SecureRoot::open_or_create(&temp.0).expect("create root");
        root.create_dir_all("directory").expect("create directory");
        root.create_file_if_absent("directory/secret-name", b"immutable")
            .expect("create leaf");
        let limits = DescriptorCensusLimitsV1::bounded(
            1,
            100,
            MAX_FILE_BYTES,
            MAX_FILE_BYTES,
            MAX_PATH_BYTES,
            MAX_COMPONENT_BYTES,
        )
        .expect("bounded census limits");
        let error = match root.descriptor_anchored_census(limits, &mut TestCensusFence::new()) {
            Ok(_) => panic!("leaf beyond depth must refuse"),
            Err(error) => error,
        };
        assert!(matches!(error, SecureFsError::CensusRefused));
        assert_eq!(error.to_string(), "descriptor-anchored census refused");
        assert!(!error.to_string().contains("secret-name"));
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
