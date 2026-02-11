use crate::cgp_patterns::is_cgp_diagnostic;
use crate::diagnostic_db::DiagnosticDatabase;
use crate::error_formatting::render_diagnostic_plain;
use cargo_metadata::Message;
use std::fs::File;
use std::io::BufReader;

/// Helper function to run a CGP error test from a JSON file
pub fn test_cgp_error_from_json(json_filename: &str, test_name: &str) -> Vec<String> {
    // Read the JSON fixture (newline-delimited JSON)
    let json_path = format!(
        "{}/../examples/src/{}",
        env!("CARGO_MANIFEST_DIR"),
        json_filename
    );

    println!("\n=== Testing {} ===", test_name);
    println!("Reading JSON from: {}", json_path);
    let file =
        File::open(&json_path).unwrap_or_else(|_| panic!("Failed to open {}", json_filename));

    let reader = BufReader::new(file);

    let mut output_lines = Vec::new();

    let mut db = DiagnosticDatabase::new();

    for message in Message::parse_stream(reader) {
        if let Message::CompilerMessage(msg) = message.expect("Failed to parse message") {
            if is_cgp_diagnostic(&msg.message) {
                db.add_diagnostic(&msg);
            }
        }
    }

    let cgp_diagnostics = db.render_cgp_diagnostics();
    for diagnostic in cgp_diagnostics {
        let rendered = render_diagnostic_plain(&diagnostic);
        println!("{}", rendered);
        output_lines.push(rendered);
    }

    // Return the output for snapshot testing
    output_lines
}
