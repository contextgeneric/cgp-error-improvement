use std::env;

use anyhow::{Result, bail};
use cargo_cgp::run_check::{OutputFormat, run_check};

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
        Some("check") => {
            // Parse the remaining arguments to extract --message-format
            let remaining_args: Vec<String> = args.iter().skip(3).cloned().collect();
            let (output_format, passthrough_args) = parse_message_format(&remaining_args);
            run_check(output_format, passthrough_args)?;
        }
        Some(other) => bail!("Unknown subcommand: {}", other),
        None => bail!("Usage: cargo cgp check"),
    }

    Ok(())
}

/// Parse --message-format argument and determine output format
/// Returns (OutputFormat, remaining args to pass to cargo)
fn parse_message_format(args: &[String]) -> (OutputFormat, Vec<String>) {
    let mut output_format = OutputFormat::Human; // default
    let mut passthrough_args = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if arg == "--message-format" {
            // Next arg is the format
            if i + 1 < args.len() {
                let format_str = &args[i + 1];
                output_format = parse_format_string(format_str);
                // Don't pass --message-format to cargo since we'll handle it
                i += 2;
                continue;
            }
        } else if arg.starts_with("--message-format=") {
            // Format is in the same arg
            let format_str = arg.trim_start_matches("--message-format=");
            output_format = parse_format_string(format_str);
            // Don't pass --message-format to cargo
            i += 1;
            continue;
        }

        // Pass through other arguments
        passthrough_args.push(arg.clone());
        i += 1;
    }

    (output_format, passthrough_args)
}

/// Parse format string to determine output format
fn parse_format_string(format_str: &str) -> OutputFormat {
    // The format can be comma-separated values like "json,diagnostic-rendered-ansi"
    let formats: Vec<&str> = format_str.split(',').map(|s| s.trim()).collect();

    // Check if any format indicates JSON output
    for format in &formats {
        if format.starts_with("json") {
            return OutputFormat::Json;
        } else if format == &"short" {
            return OutputFormat::Short;
        }
    }

    // Default to human readable
    OutputFormat::Human
}
