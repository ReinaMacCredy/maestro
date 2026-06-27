#[test]
fn card_store_atomic_write_helper_does_not_call_blocking_sync() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest_dir.join("src/foundation/core/safe_write.rs"))
        .expect("invariant: safe_write source should be readable");

    assert!(
        !source.contains(".sync_all()"),
        "card-store atomic writes must not call sync_all; it can block indefinitely in uninterruptible I/O"
    );
    assert!(
        !source.contains("sync_parent_dir("),
        "card-store atomic writes must not fsync the parent directory on the hot path"
    );
}
