use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod atomic_publication;
mod canonical_store;
mod store_full_export;
mod store_safety;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: system clock should be after the Unix epoch")
            .as_nanos();
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("{prefix}-{}-{timestamp}-{counter}", process::id()));

        fs::create_dir(&path).expect("invariant: unique test temp directory should be creatable");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
