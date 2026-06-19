use anyhow::Result;

use crate::domain::lean::{self, LeanMode};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{LeanArgs, cli_run_id};

/// Execute `maestro lean [TARGET] [--card]`:
///   (none)               -> print the session's current lean mode
///   lite|full|ultra|off  -> set the session's lean mode
///   review               -> mode-adjusted reach-ladder review guidance
///   audit                -> mode-adjusted reach-ladder audit guidance
///   debt [--card]        -> list anchored `// lean:` markers (and mint cards)
///
/// The mode is keyed to this session's run dir, so concurrent sessions stay
/// independent and a fresh session starts from the `MAESTRO_LEAN` default.
pub fn run(args: LeanArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let session = cli_run_id();
    let env_default = std::env::var("MAESTRO_LEAN").ok();
    let resolve = || lean::resolve_mode(&paths, &session, env_default.as_deref());

    match args.target.as_deref().map(str::trim) {
        None => println!("{}", resolve()),
        Some("review") => print!("{}", lean::review_guidance(resolve())),
        Some("audit") => print!("{}", lean::audit_guidance(resolve())),
        Some("debt") => run_debt(&paths, args.card)?,
        Some(token) => match LeanMode::parse(token) {
            Some(mode) => {
                lean::write_mode(&paths, &session, mode)?;
                println!("{mode}");
            }
            None => anyhow::bail!(
                "unknown lean target `{token}`; expected one of: lite, full, ultra, off, review, audit, debt"
            ),
        },
    }
    Ok(())
}

/// List the anchored `// lean:` markers across the tree, and with `--card` mint
/// one deduped task card per marker.
fn run_debt(paths: &MaestroPaths, mint: bool) -> Result<()> {
    let markers = lean::harvest(paths)?;
    if markers.is_empty() {
        println!("no lean markers");
        return Ok(());
    }
    for marker in &markers {
        println!("{}:{}  {}", marker.file, marker.line, marker.text);
    }
    println!("{} marker(s)", markers.len());

    if mint {
        let outcome = lean::mint_cards(paths, &markers)?;
        for (id, marker) in &outcome.minted {
            println!("minted {id}  ({}:{})", marker.file, marker.line);
        }
        println!("minted {}, deduped {}", outcome.minted.len(), outcome.deduped);
    }
    Ok(())
}
