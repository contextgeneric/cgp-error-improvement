use std::env;

use anyhow::{Result, bail};
use cargo_cgp::run_check::run_check;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Cargo invokes us as: cargo-cgp cgp <subcommand> [args...]
    // We want to support: cargo cgp check
    if args.len() < 2 {
        bail!("Usage: cargo cgp check");
    }

    // Skip program name and "cgp" argument
    let subcommand = args.get(2);

    match subcommand.map(|s| s.as_str()) {
        Some("check") => run_check()?,
        Some(other) => bail!("Unknown subcommand: {}", other),
        None => bail!("Usage: cargo cgp check"),
    }

    Ok(())
}
