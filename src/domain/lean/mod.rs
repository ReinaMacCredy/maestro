//! Lean: the always-on reach-ladder's session-scoped tooling.
//!
//! [`mode`] holds the per-session strictness dial (lite/full/ultra/off) that
//! tunes how hard the reach-ladder is enforced. [`debt`] harvests inline
//! `// lean:` markers from the tree into a ledger and mints deduped task cards
//! from them.

mod debt;
mod mode;

pub use debt::{Marker, MintOutcome, harvest, marker_text, mint_cards};
pub use mode::{LeanMode, read_mode, resolve_mode, write_mode};
