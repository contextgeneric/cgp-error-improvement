use std::io::BufReader;
use std::process::{Command, Stdio};

use crate::diagnostic_db::DiagnosticDatabase;
use crate::render::{RenderMode, render_message, render_message_with_mode};
use anyhow::{Context, Result};
use cargo_metadata::Message;

/// Output format for cargo-cgp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable format (default, uses miette in TTY mode)
    Human,
    /// Short human-readable format
    Short,
    /// JSON format (emits transformed JSON messages)
    Json,
}

pub fn run_check(output_format: OutputFormat, passthrough_args: Vec<String>) -> Result<()> {
    // Spawn cargo check with JSON output
    // We always need JSON internally to analyze the errors
    let mut child = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .args(&passthrough_args)
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

    // For JSON mode, collect non-CGP messages to emit later
    let mut non_cgp_messages = Vec::new();

    // Process and render each message based on format
    for message in messages {
        let message = message.context("Failed to parse JSON message from cargo")?;

        match output_format {
            OutputFormat::Json => {
                // For JSON output, collect messages instead of printing immediately
                if let Some(non_cgp_msg) =
                    render_message_with_mode(&message, &mut db, RenderMode::Json)
                {
                    non_cgp_messages.push(non_cgp_msg);
                }
            }
            OutputFormat::Human | OutputFormat::Short => {
                // For human output, render immediately (non-CGP messages print directly)
                render_message(&message, &mut db);
            }
        }
    }

    // After all messages are processed, render all CGP errors
    match output_format {
        OutputFormat::Json => {
            // First emit all non-CGP messages
            for msg in non_cgp_messages {
                if let Ok(json) = serde_json::to_string(&msg) {
                    println!("{}", json);
                }
            }

            // Then emit transformed CGP messages as JSON CompilerMessage objects
            let compiler_messages = db.render_compiler_messages();
            for msg in compiler_messages {
                if let Ok(json) = serde_json::to_string(&msg) {
                    println!("{}", json);
                }
            }
        }
        OutputFormat::Human | OutputFormat::Short => {
            // Render as human-readable text with miette (if TTY)
            // For tests and non-TTY, this will use plain text
            let cgp_errors = db.render_cgp_errors_with_miette();
            for error in cgp_errors {
                println!("{}", error);
            }
        }
    }

    // Wait for cargo check to complete
    let status = child.wait().context("Failed to wait for cargo check")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
