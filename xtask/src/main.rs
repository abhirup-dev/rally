use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("ci") => run_ci(),
        Some("schema") => {
            eprintln!("xtask schema: not yet implemented");
            ExitCode::SUCCESS
        }
        Some(t) => {
            eprintln!("unknown task: {t}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <task>\n\ntasks:\n  ci       Run CI quality gates\n  schema   Generate schema (not yet implemented)");
            ExitCode::FAILURE
        }
    }
}

fn run_ci() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        ("cargo fmt --all --check", &["cargo", "fmt", "--all", "--check"]),
        (
            "cargo clippy --all-targets -- -D warnings",
            &["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        ),
        ("cargo test --workspace", &["cargo", "test", "--workspace"]),
    ];

    let mut failed = false;
    for (label, args) in steps {
        eprintln!("\n=== {label} ===");
        let status = Command::new(args[0])
            .args(&args[1..])
            .status();
        match status {
            Ok(s) if s.success() => eprintln!("  ✓ passed"),
            Ok(s) => {
                eprintln!("  ✗ failed (exit {})", s.code().unwrap_or(-1));
                failed = true;
            }
            Err(e) => {
                eprintln!("  ✗ failed to run: {e}");
                failed = true;
            }
        }
    }

    if failed {
        eprintln!("\n=== CI FAILED ===");
        ExitCode::FAILURE
    } else {
        eprintln!("\n=== CI PASSED ===");
        ExitCode::SUCCESS
    }
}
