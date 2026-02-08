use std::env;
use std::io::BufReader;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use cargo_cgp::render::render_message;
use cargo_metadata::Message;

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

fn run_check() -> Result<()> {
    // Spawn cargo check with JSON output
    let mut child = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn cargo check")?;

    // Get stdout handle
    let stdout = child
        .stdout
        .take()
        .context("Failed to capture stdout from cargo check")?;

    // Parse JSON messages from stdout
    let reader = BufReader::new(stdout);
    let messages = Message::parse_stream(reader);

    // Process and render each message
    for message in messages {
        let message = message.context("Failed to parse JSON message from cargo")?;
        render_message(&message);
    }

    // Wait for cargo check to complete
    let status = child.wait().context("Failed to wait for cargo check")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
