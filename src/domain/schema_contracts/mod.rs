//! Schema contract kernel (WS5 / D6.2-B + D6.3).
//!
//! `embedded/schemas/` ships one reviewable pack per artifact family; this
//! module parses them ([`catalog`]), proves them consistent with the Rust
//! constants ([`validate`]), and serves the bounded-version classification
//! ([`VersionClass`]) that domain gate sites consume: a found stamp is either
//! in the family's read set, a named legacy version with an explicit migrate
//! route, or unknown and refused.

pub mod catalog;
pub mod validate;

pub use catalog::{SchemaPack, VersionClass, pack, packs};
