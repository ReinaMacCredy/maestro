use serde_json::{Map, Value, json};

use maestro::foundation::core::managed_blocks::{
    ManagedBlockFormat, remove_managed_block, remove_managed_json_keys, upsert_managed_block,
    upsert_managed_json_keys,
};

#[test]
fn markdown_block_is_created_for_missing_file() {
    let output = upsert_managed_block(
        None,
        ManagedBlockFormat::Markdown,
        "Read .maestro/HARNESS.md",
    );

    assert_eq!(
        output,
        "<!-- maestro:start -->\nRead .maestro/HARNESS.md\n<!-- maestro:end -->\n"
    );
}

#[test]
fn markdown_block_preserves_user_content_and_replaces_existing_managed_content() {
    let existing = "# User notes\n\n<!-- maestro:start -->\nold\n<!-- maestro:end -->\n\nKeep me\n";

    let output = upsert_managed_block(
        Some(existing),
        ManagedBlockFormat::Markdown,
        "new managed content",
    );

    assert_eq!(
        output,
        "# User notes\n\n<!-- maestro:start -->\nnew managed content\n<!-- maestro:end -->\n\nKeep me\n"
    );
}

#[test]
fn deleted_markdown_block_is_reinstalled_by_append() {
    let existing = "# User notes\nKeep me\n";

    let output = upsert_managed_block(
        Some(existing),
        ManagedBlockFormat::Markdown,
        "managed again",
    );

    assert_eq!(
        output,
        "# User notes\nKeep me\n\n<!-- maestro:start -->\nmanaged again\n<!-- maestro:end -->\n"
    );
}

#[test]
fn hash_comment_block_supports_gitignore_and_toml_style_files() {
    let existing = "target/\n";

    let output = upsert_managed_block(
        Some(existing),
        ManagedBlockFormat::HashComment,
        ".maestro/runs/\n.maestro/backups/",
    );

    assert_eq!(
        output,
        "target/\n\n# >>> maestro >>>\n.maestro/runs/\n.maestro/backups/\n# <<< maestro <<<\n"
    );
}

#[test]
fn remove_managed_block_preserves_user_content() {
    let existing = "before\n\n# >>> maestro >>>\nmanaged\n# <<< maestro <<<\n\nafter\n";

    let output = remove_managed_block(existing, ManagedBlockFormat::HashComment);

    assert_eq!(output, "before\n\nafter\n");
}

#[test]
fn remove_managed_block_at_eof_preserves_single_user_newline() {
    let existing = "before\n\n# >>> maestro >>>\nmanaged\n# <<< maestro <<<\n";

    let output = remove_managed_block(existing, ManagedBlockFormat::HashComment);

    assert_eq!(output, "before\n");
}

#[test]
fn json_managed_keys_are_merged_and_manifested() {
    let mut managed = Map::new();
    managed.insert("hooks".to_string(), json!({"Stop": []}));

    let output = upsert_managed_json_keys(Some(r#"{"user":true}"#), managed)
        .expect("invariant: JSON object merge should succeed");
    let parsed: Value =
        serde_json::from_str(&output).expect("invariant: output should be valid JSON");

    assert_eq!(parsed["user"], json!(true));
    assert_eq!(parsed["hooks"], json!({"Stop": []}));
    assert_eq!(parsed["_maestro_managed_keys"], json!(["hooks"]));
}

#[test]
fn json_upsert_replaces_previous_managed_keys_without_touching_user_keys() {
    let existing = r#"{
  "user": true,
  "hooks": {"old": true},
  "_maestro_managed_keys": ["hooks"]
}"#;
    let mut managed = Map::new();
    managed.insert(
        "mcpServers".to_string(),
        json!({"maestro": {"command": "maestro"}}),
    );

    let output = upsert_managed_json_keys(Some(existing), managed)
        .expect("invariant: JSON object merge should succeed");
    let parsed: Value =
        serde_json::from_str(&output).expect("invariant: output should be valid JSON");

    assert_eq!(parsed["user"], json!(true));
    assert_eq!(parsed["hooks"], json!({"old": true}));
    assert_eq!(
        parsed["mcpServers"],
        json!({"maestro": {"command": "maestro"}})
    );
    assert_eq!(parsed["_maestro_managed_keys"], json!(["mcpServers"]));
}

#[test]
fn json_managed_keys_are_removed_reversibly() {
    let existing = r#"{
  "user": true,
  "hooks": {"Stop": []},
  "_maestro_managed_keys": ["hooks"]
}"#;

    let output = remove_managed_json_keys(existing, &["hooks"])
        .expect("invariant: managed JSON removal should succeed");
    let parsed: Value =
        serde_json::from_str(&output).expect("invariant: output should be valid JSON");

    assert_eq!(parsed, json!({"user": true}));
}

#[test]
fn json_managed_key_removal_ignores_tampered_manifest_entries() {
    let existing = r#"{
  "user": true,
  "hooks": {"Stop": []},
  "_maestro_managed_keys": ["hooks", "user"]
}"#;

    let output = remove_managed_json_keys(existing, &["hooks"])
        .expect("invariant: managed JSON removal should succeed");
    let parsed: Value =
        serde_json::from_str(&output).expect("invariant: output should be valid JSON");

    assert_eq!(parsed, json!({"user": true}));
}

#[test]
fn removing_managed_block_preserves_unrelated_user_spacing() {
    let existing = "intro\n\n\nbefore\n\n# >>> maestro >>>\nmanaged\n# <<< maestro <<<\n\nafter\n";

    let output = remove_managed_block(existing, ManagedBlockFormat::HashComment);

    assert_eq!(output, "intro\n\n\nbefore\n\nafter\n");
}

#[test]
fn json_mirror_rejects_non_object_content() {
    let error = upsert_managed_json_keys(Some("[]"), Map::new())
        .expect_err("invariant: non-object JSON mirror should fail");

    assert_eq!(
        error.to_string(),
        "managed JSON mirror must be a top-level object"
    );
}
