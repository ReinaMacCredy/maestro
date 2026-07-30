//! Bundled DESIGN.md resources, served from the binary.
//!
//! Maestro ships one neutral, non-brand default template and a vendored catalog
//! of opt-in `awesome:<site>` templates copied from a pinned
//! `VoltAgent/awesome-design-md` commit. `maestro design init` writes one of
//! these files only by explicit user action; once written, the repository-root
//! `DESIGN.md` is user-owned.

use anyhow::{Context, Result};
use include_dir::{Dir, include_dir};
use serde::Deserialize;

/// The bundled neutral design templates.
static DESIGN_STYLES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/design/styles");

/// The vendored awesome-design-md snapshot.
static AWESOME_DESIGN_MD_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/embedded/design/vendor/awesome-design-md");

const DESIGN_FILE: &str = "DESIGN.md";
const DEFAULT_STYLE: &str = "neutral";
const AWESOME_PREFIX: &str = "awesome:";
pub const AWESOME_DESIGN_MD_REPOSITORY: &str = "https://github.com/VoltAgent/awesome-design-md";

/// A shipped DESIGN.md style that can be copied into a repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesignStyle {
    /// CLI token, e.g. `neutral` or `awesome:linear.app`.
    pub token: String,
    /// Human-readable source label for dry-runs and list output.
    pub source_label: String,
    /// Embedded resource path relative to `embedded/design`.
    pub embedded_path: String,
    /// True when the style comes from the vendored awesome-design-md catalog.
    pub is_vendor: bool,
}

/// A served DESIGN.md style and its verbatim contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServedDesign {
    pub style: DesignStyle,
    pub contents: &'static str,
}

/// Source metadata for the vendored awesome-design-md catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AwesomeDesignMdManifest {
    pub source: AwesomeDesignMdSource,
    pub copied_files: Vec<AwesomeDesignMdFile>,
}

/// Upstream source metadata for the vendored catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AwesomeDesignMdSource {
    pub repository: String,
    pub commit: String,
    pub license: String,
    pub license_file: String,
    pub upstream_path: String,
}

/// One copied upstream DESIGN.md file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AwesomeDesignMdFile {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

/// The default non-brand style token.
pub fn default_style() -> &'static str {
    DEFAULT_STYLE
}

/// Every shipped style token, sorted.
pub fn available_style_tokens() -> Vec<String> {
    styles().into_iter().map(|style| style.token).collect()
}

/// Every shipped style, sorted by token.
pub fn styles() -> Vec<DesignStyle> {
    let mut styles = Vec::new();
    styles.extend(builtin_styles());
    styles.extend(awesome_styles());
    styles.sort_by(|left, right| left.token.cmp(&right.token));
    styles
}

/// Serve a shipped style verbatim. Unknown tokens fail loud with available choices.
pub fn serve(style: Option<&str>) -> Result<ServedDesign> {
    let token = style.unwrap_or(DEFAULT_STYLE);
    if token == DEFAULT_STYLE {
        let contents = builtin_contents(DEFAULT_STYLE)?;
        return Ok(ServedDesign {
            style: builtin_style(DEFAULT_STYLE),
            contents,
        });
    }

    if let Some(slug) = token.strip_prefix(AWESOME_PREFIX) {
        let contents = awesome_contents(slug)?;
        return Ok(ServedDesign {
            style: awesome_style(slug),
            contents,
        });
    }

    bail_unknown_style(token)
}

/// Parse the vendored awesome-design-md manifest.
pub fn awesome_manifest() -> Result<AwesomeDesignMdManifest> {
    serde_yaml::from_str(awesome_manifest_yaml())
        .context("failed to parse awesome-design-md manifest")
}

/// The shipped manifest text for auditing the vendored catalog.
pub fn awesome_manifest_yaml() -> &'static str {
    AWESOME_DESIGN_MD_DIR
        .get_file(AWESOME_DESIGN_MD_DIR.path().join("manifest.yml"))
        .and_then(|file| file.contents_utf8())
        .expect("invariant: awesome-design-md manifest.yml is embedded and UTF-8")
}

/// The shipped upstream MIT license text for the vendored catalog.
pub fn awesome_license() -> &'static str {
    AWESOME_DESIGN_MD_DIR
        .get_file(AWESOME_DESIGN_MD_DIR.path().join("LICENSE"))
        .and_then(|file| file.contents_utf8())
        .expect("invariant: awesome-design-md LICENSE is embedded and UTF-8")
}

fn builtin_styles() -> Vec<DesignStyle> {
    DESIGN_STYLES_DIR
        .dirs()
        .filter_map(|dir| dir.path().file_name()?.to_str().map(builtin_style))
        .collect()
}

fn builtin_style(slug: &str) -> DesignStyle {
    DesignStyle {
        token: slug.to_string(),
        source_label: format!("maestro:{slug}"),
        embedded_path: format!("styles/{slug}/{DESIGN_FILE}"),
        is_vendor: false,
    }
}

fn builtin_contents(slug: &str) -> Result<&'static str> {
    DESIGN_STYLES_DIR
        .get_file(DESIGN_STYLES_DIR.path().join(slug).join(DESIGN_FILE))
        .and_then(|file| file.contents_utf8())
        .ok_or_else(|| unknown_style_error(slug))
}

fn awesome_styles() -> Vec<DesignStyle> {
    awesome_design_root()
        .dirs()
        .filter_map(|dir| dir.path().file_name()?.to_str().map(awesome_style))
        .collect()
}

fn awesome_style(slug: &str) -> DesignStyle {
    DesignStyle {
        token: format!("{AWESOME_PREFIX}{slug}"),
        source_label: format!("awesome-design-md:{slug}"),
        embedded_path: format!("vendor/awesome-design-md/design-md/{slug}/{DESIGN_FILE}"),
        is_vendor: true,
    }
}

fn awesome_contents(slug: &str) -> Result<&'static str> {
    awesome_design_root()
        .get_file(awesome_design_root().path().join(slug).join(DESIGN_FILE))
        .and_then(|file| file.contents_utf8())
        .ok_or_else(|| unknown_style_error(&format!("{AWESOME_PREFIX}{slug}")))
}

fn awesome_design_root() -> &'static Dir<'static> {
    AWESOME_DESIGN_MD_DIR
        .get_dir(AWESOME_DESIGN_MD_DIR.path().join("design-md"))
        .expect("invariant: awesome-design-md design-md directory is embedded")
}

fn bail_unknown_style(token: &str) -> Result<ServedDesign> {
    Err(unknown_style_error(token))
}

fn unknown_style_error(token: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "unknown design style \"{token}\"; available: {}",
        available_style_tokens().join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_is_neutral() {
        assert_eq!(default_style(), "neutral");
        assert_eq!(serve(None).unwrap().style.token, "neutral");
    }

    #[test]
    fn lists_neutral_and_every_vendored_awesome_style() {
        let tokens = available_style_tokens();
        assert!(tokens.contains(&"neutral".to_string()), "{tokens:?}");
        let manifest = awesome_manifest().unwrap();
        for file in manifest.copied_files {
            let slug = file
                .path
                .strip_prefix("design-md/")
                .and_then(|rest| rest.strip_suffix("/DESIGN.md"))
                .unwrap_or_else(|| panic!("unexpected manifest path {}", file.path));
            let token = format!("awesome:{slug}");
            assert!(tokens.contains(&token), "missing {token}");
        }
    }

    #[test]
    fn serves_every_vendored_awesome_style_byte_identical_to_the_embedded_file() {
        for style in styles().into_iter().filter(|style| style.is_vendor) {
            let slug = style.token.strip_prefix(AWESOME_PREFIX).unwrap();
            let embedded = awesome_design_root()
                .get_file(awesome_design_root().path().join(slug).join(DESIGN_FILE))
                .and_then(|file| file.contents_utf8())
                .unwrap_or_else(|| panic!("embedded DESIGN.md for {slug} is missing"));
            assert_eq!(serve(Some(&style.token)).unwrap().contents, embedded);
        }
    }

    #[test]
    fn manifest_is_pinned_to_the_accepted_upstream_commit() {
        let manifest = awesome_manifest().unwrap();
        assert_eq!(manifest.source.repository, AWESOME_DESIGN_MD_REPOSITORY);
        assert_eq!(
            manifest.source.commit,
            "664b3e78fd1a298ba11973822da988483256d4b4"
        );
        assert_eq!(manifest.source.license, "MIT");
        assert_eq!(manifest.source.license_file, "LICENSE");
        assert_eq!(manifest.source.upstream_path, "design-md/*/DESIGN.md");
        assert_eq!(manifest.copied_files.len(), 74);
    }

    #[test]
    fn unknown_style_is_a_loud_error_listing_available_tokens() {
        let error = serve(Some("awesome:not-real")).unwrap_err().to_string();
        assert!(error.contains("awesome:not-real"), "{error}");
        assert!(error.contains("neutral"), "{error}");
        assert!(error.contains("awesome:voltagent"), "{error}");
    }
}
