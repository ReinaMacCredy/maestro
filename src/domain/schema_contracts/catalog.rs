//! Embedded schema-contract catalog: the parsed view of `embedded/schemas/`.
//!
//! Each artifact family ships a pack (`current.yaml`, `supported.yaml`,
//! `retired.yaml`, `fixtures/`) that is the reviewable source of truth for its
//! field contract, bounded read set, legacy migrate routes, and never-reuse
//! names. Rust stays the trusted interpreter: this module only parses and
//! serves the packs; `validate` proves them consistent with the Rust
//! constants, and domain gate sites consume [`SchemaPack::classify`].

use std::sync::OnceLock;

use include_dir::{Dir, include_dir};
use serde::Deserialize;

/// The shipped schema contract packs, one directory per artifact family.
static SCHEMAS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/schemas");

static CATALOG: OnceLock<Vec<SchemaPack>> = OnceLock::new();

/// One artifact family's parsed schema pack.
#[derive(Clone, Debug)]
pub struct SchemaPack {
    /// Family key: the pack's directory name, e.g. `task` or `run-event`.
    pub family: &'static str,
    /// The current file contract(s) for the family.
    pub current: CurrentContracts,
    /// Bounded read set and explicit legacy routes.
    pub supported: SupportedVersions,
    /// Removed/reserved names that must never be reused.
    pub retired: RetiredNames,
    /// Fixture files shipped with the pack, paths relative to the pack dir.
    pub fixtures: Vec<Fixture>,
}

/// Parsed `current.yaml`: the family's file contract(s).
#[derive(Clone, Debug, Deserialize)]
pub struct CurrentContracts {
    /// Family key; must match the pack directory name.
    pub family: String,
    /// One contract per stamped file shape the family persists.
    pub contracts: Vec<ContractSpec>,
}

/// One stamped file contract inside a pack.
#[derive(Clone, Debug, Deserialize)]
pub struct ContractSpec {
    /// Contract name, unique within the family.
    pub name: String,
    /// The current version stamp for this contract.
    pub schema_version: String,
    /// Where the stamp lives: `file`, `payload`, `envelope`, or `line`.
    pub stamp: String,
    /// On-disk shape: `file`, `card-extra`, `card-entry`, or `line`.
    pub carrier: String,
    /// Serialization: `yaml`, `json`, or `jsonl`.
    pub format: String,
    /// Human description of where the artifact lives.
    pub home: String,
    /// The Rust model that owns trusted interpretation.
    pub model: String,
    /// Every persisted field name the contract allows.
    pub fields: Vec<String>,
}

/// Parsed `supported.yaml`: what this binary reads, writes, and routes.
#[derive(Clone, Debug, Deserialize)]
pub struct SupportedVersions {
    /// Family key; must match the pack directory name.
    pub family: String,
    /// Versions readable in memory. Day one this is exactly the current
    /// stamp(s); a future version bump grows it together with a converter.
    pub read: Vec<String>,
    /// The version state-changing commands write.
    pub write: String,
    /// Named legacy versions with explicit migrate routes; never normalized
    /// in memory (locked D6.1 change policy).
    #[serde(default)]
    pub legacy: Vec<LegacyRoute>,
}

/// One legacy version and the explicit command that upgrades it.
#[derive(Clone, Debug, Deserialize)]
pub struct LegacyRoute {
    /// The legacy version stamp as it appears on disk.
    pub version: String,
    /// The explicit migrate route the refusal message points at.
    pub route: String,
}

/// Parsed `retired.yaml`: names that must never be reused.
#[derive(Clone, Debug, Deserialize)]
pub struct RetiredNames {
    /// Family key; must match the pack directory name.
    pub family: String,
    /// Retired/reserved version stamps.
    pub versions: Vec<String>,
    /// Retired field names.
    pub fields: Vec<String>,
}

/// One fixture file shipped with a pack.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// Path relative to the pack directory, e.g. `fixtures/current-full.yaml`.
    pub relative_path: &'static str,
    /// Embedded file bytes.
    pub contents: &'static [u8],
}

/// How a found version stamp relates to a family's declared read set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersionClass {
    /// In the family's in-memory read set: proceed.
    Supported,
    /// A named legacy version: refuse with the pack's explicit migrate route.
    Legacy {
        /// The migrate route declared in `supported.yaml`.
        route: &'static str,
    },
    /// Undeclared (newer, foreign, or corrupted): refuse, naming the read set.
    Unknown,
}

impl SchemaPack {
    /// Classify a found version stamp against this family's declared versions.
    pub fn classify(&'static self, found: &str) -> VersionClass {
        if self.supported.read.iter().any(|version| version == found) {
            return VersionClass::Supported;
        }
        match self
            .supported
            .legacy
            .iter()
            .find(|legacy| legacy.version == found)
        {
            Some(legacy) => VersionClass::Legacy {
                route: legacy.route.as_str(),
            },
            None => VersionClass::Unknown,
        }
    }

    /// The contract whose current stamp is `version`, if any.
    pub fn contract_for_version(&self, version: &str) -> Option<&ContractSpec> {
        self.current
            .contracts
            .iter()
            .find(|contract| contract.schema_version == version)
    }
}

/// Every shipped schema pack, sorted by family.
pub fn packs() -> &'static [SchemaPack] {
    CATALOG.get_or_init(build_catalog).as_slice()
}

/// The schema pack for one artifact family.
pub fn pack(family: &str) -> Option<&'static SchemaPack> {
    packs().iter().find(|pack| pack.family == family)
}

fn build_catalog() -> Vec<SchemaPack> {
    let mut packs = SCHEMAS_DIR
        .dirs()
        .map(|dir| {
            let family = dir
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .expect("invariant: an embedded schema pack directory has a UTF-8 name");
            SchemaPack {
                family,
                current: parse_pack_file(dir, family, "current.yaml"),
                supported: parse_pack_file(dir, family, "supported.yaml"),
                retired: parse_pack_file(dir, family, "retired.yaml"),
                fixtures: collect_fixtures(dir),
            }
        })
        .collect::<Vec<_>>();
    packs.sort_by_key(|pack| pack.family);
    packs
}

fn parse_pack_file<T: serde::de::DeserializeOwned>(
    dir: &'static Dir<'static>,
    family: &str,
    name: &str,
) -> T {
    let file = dir
        .get_file(dir.path().join(name))
        .unwrap_or_else(|| panic!("invariant: schema pack {family} ships {name}"));
    let raw = file
        .contents_utf8()
        .unwrap_or_else(|| panic!("invariant: schema pack file {family}/{name} is UTF-8"));
    serde_yaml::from_str(raw).unwrap_or_else(|error| {
        panic!("invariant: schema pack file {family}/{name} parses: {error}")
    })
}

fn collect_fixtures(dir: &'static Dir<'static>) -> Vec<Fixture> {
    let Some(fixtures_dir) = dir.get_dir(dir.path().join("fixtures")) else {
        return Vec::new();
    };
    let mut fixtures = fixtures_dir
        .files()
        .map(|file| {
            let relative_path = file
                .path()
                .strip_prefix(dir.path())
                .ok()
                .and_then(|path| path.to_str())
                .expect("invariant: a schema fixture lives under its pack with a UTF-8 path");
            Fixture {
                relative_path,
                contents: file.contents(),
            }
        })
        .collect::<Vec<_>>();
    fixtures.sort_by_key(|fixture| fixture.relative_path);
    fixtures
}
