//! Development tasks for the workspace, run with `cargo xtask <task>`.
//!
//! - `uml`: regenerates the class diagram in docs/CLASS_DIAGRAM.md from the
//!   source of `nutrition-core` (D22).
//! - `uml --check`: fails when the committed diagram does not match the source.

mod uml;

use std::process::ExitCode;

/// Runs the task named on the command line and reports its outcome.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = uml::workspace_root();
    let outcome = match args.as_slice() {
        ["uml"] => uml::write(root).map(|changed| {
            let status = if changed {
                "updated"
            } else {
                "already up to date"
            };
            println!("{} {status}", uml::DOC);
        }),
        ["uml", "--check"] => uml::check(root),
        _ => Err("usage: cargo xtask uml [--check]".into()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
