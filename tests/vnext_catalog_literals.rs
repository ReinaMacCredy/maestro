use std::path::Path;
use std::process::{Command, Output};

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"))
}

fn assert_success(label: &str, output: Output) {
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn predecessor_catalog_literals_are_exactly_reproduced() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));

    assert_success(
        "Stage-0 predecessor catalog reproduction",
        run(
            repo,
            "python3",
            &["tools/vnext_contracts/catalogs/verify_predecessors.py"],
        ),
    );
}

#[test]
fn efa0_catalog_literals_are_reproducible_and_semantically_closed() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated = "contracts/vnext/catalogs/generated";

    assert_success(
        "Stage-0 catalog reproducibility check",
        run(
            repo,
            "python3",
            &["tools/vnext_contracts/catalogs/build.py", "--check"],
        ),
    );
    assert_success(
        "Stage-0 independent semantic validation",
        run(
            repo,
            "python3",
            &[
                "tools/vnext_contracts/catalogs/validate.py",
                "--generated",
                generated,
                "--mutants",
            ],
        ),
    );
}
