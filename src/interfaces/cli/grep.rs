use anyhow::Result;

use crate::domain::search;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::GrepArgs;

pub fn run(args: GrepArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let query = args.query.join(" ");
    let envelope = search::grep(&paths, &query);

    if args.json {
        println!("{}", serde_json::to_string(&envelope)?);
    } else if envelope.ok {
        render_human(&envelope.hits);
    } else {
        for diagnostic in &envelope.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        std::process::exit(2);
    }
    Ok(())
}

fn render_human(hits: &[search::SearchHit]) {
    if hits.is_empty() {
        println!("no hits");
        return;
    }
    for hit in hits {
        let target = hit_target(hit);
        println!(
            "{}. {}:{} {} score={:.2} {}",
            hit.rank,
            hit.corpus.as_str(),
            hit.kind,
            target,
            hit.score,
            hit.title
        );
        println!("  {}", hit.snippet);
        if let Some(opener) = &hit.opener {
            println!("  open: {opener}");
        }
    }
}

fn hit_target(hit: &search::SearchHit) -> String {
    match (&hit.path, hit.line) {
        (Some(path), Some(line)) if hit.corpus == search::types::SearchCorpus::Source => {
            format!("{path}:{line}")
        }
        _ => hit.id.clone(),
    }
}
