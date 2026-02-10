use std::env;
use std::io::BufReader;
use std::process::{Command, Stdio};

use crate::diagnostic_db::DiagnosticDatabase;
use crate::error_formatting::{is_terminal, render_diagnostic_graphical, render_diagnostic_plain};
use crate::render::render_message;
use anyhow::{Context, Result};
use cargo_metadata::Message;

pub fn run_check() -> Result<()> {
    // Get any additional arguments to pass through to cargo
    let args: Vec<String> = env::args().skip(3).collect();

    // Spawn cargo check with JSON output
    let mut child = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Capture stderr to prevent progress bar interference
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

    // Create database to collect CGP diagnostics
    let mut db = DiagnosticDatabase::new();

    // Process and render each message
    for message in messages {
        let message = message.context("Failed to parse JSON message from cargo")?;
        render_message(&message, &mut db);
    }

    // After all messages are processed, render all CGP errors
    // Use colorful output if in terminal, plain text otherwise
    let use_color = is_terminal();
    let cgp_diagnostics = db.render_cgp_diagnostics();

    for diagnostic in cgp_diagnostics {
        let rendered = if use_color {
            render_diagnostic_graphical(&diagnostic)
        } else {
            render_diagnostic_plain(&diagnostic)
        };
        println!("{}", rendered);
    }

    // Wait for cargo check to complete
    let status = child.wait().context("Failed to wait for cargo check")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
