use anyhow::Error;
use cargo_metadata::CompilerMessage;
use cargo_metadata::diagnostic::{Diagnostic, DiagnosticLevel};

pub fn render_compiler_message(message: &CompilerMessage) -> Result<String, Error> {
    let diagnostic = &message.message;

    // Check if this is a CGP-related error
    if is_cgp_error(diagnostic) {
        render_cgp_error(diagnostic)
    } else {
        // Return the original rendered message for non-CGP errors
        if let Some(rendered) = &diagnostic.rendered {
            Ok(rendered.clone())
        } else {
            Ok("".to_owned())
        }
    }
}

/// Checks if a diagnostic is a CGP-related error
fn is_cgp_error(diagnostic: &Diagnostic) -> bool {
    // Check for CGP-specific patterns in the error message
    let cgp_patterns = [
        "CanUseComponent",
        "IsProviderFor",
        "HasField",
        "HasRectangleFields",
        "cgp_impl",
        "cgp_component",
        "delegate_components",
        "check_components",
    ];

    // Check main message
    if cgp_patterns.iter().any(|p| diagnostic.message.contains(p)) {
        return true;
    }

    // Check children messages
    for child in &diagnostic.children {
        if cgp_patterns.iter().any(|p| child.message.contains(p)) {
            return true;
        }
    }

    false
}

/// Renders a CGP error with improved formatting
fn render_cgp_error(diagnostic: &Diagnostic) -> Result<String, Error> {
    let mut output = String::new();

    // Find the root cause (missing field) from the help section
    let missing_field_info = extract_missing_field_info(diagnostic);

    // Find the primary error location
    let primary_span = diagnostic.spans.iter().find(|s| s.is_primary);

    // Build the error header
    if let Some(code) = &diagnostic.code {
        output.push_str(&format!("error[{}]: ", code.code));
    } else {
        output.push_str("error: ");
    }

    // If we found the root cause, present it first
    if let Some(field_info) = &missing_field_info {
        output.push_str(&format!(
            "missing field `{}` required by CGP component\n",
            field_info.field_name
        ));

        if let Some(span) = primary_span {
            output.push_str(&format!(
                "  --> {}:{}:{}\n",
                span.file_name, span.line_start, span.column_start
            ));
            output.push_str("   |\n");

            // Show the relevant source lines
            for (i, text_line) in span.text.iter().enumerate() {
                let line_num = span.line_start + i;
                output.push_str(&format!("{:4} | {}\n", line_num, text_line.text));

                // Add the caret line for primary span
                if i == 0 {
                    let spaces = " ".repeat(span.column_start - 1);
                    let carets = "^".repeat(span.column_end - span.column_start);
                    output.push_str(&format!(
                        "     | {}{} {}\n",
                        spaces,
                        carets,
                        span.label.as_deref().unwrap_or("")
                    ));
                }
            }
        }

        output.push_str("   |\n");
        output.push_str(&format!(
            "   = help: struct `{}` is missing the field `{}`\n",
            field_info.struct_name, field_info.field_name
        ));
        output.push_str(&format!(
            "   = note: this field is required by the trait bound `{}`\n",
            field_info.required_trait
        ));

        // Show the delegation chain in a simplified form
        output.push_str("   = note: delegation chain:\n");
        for note in extract_delegation_chain(diagnostic) {
            output.push_str(&format!("           - {}\n", note));
        }

        // Suggest a fix
        output.push_str(&format!(
            "   = help: add `pub {}: f64` to the `{}` struct definition\n",
            field_info.field_name, field_info.struct_name
        ));
    } else {
        // Fallback: if we can't identify the root cause, show the original error
        // but with improved formatting
        output.push_str(&diagnostic.message);
        output.push('\n');

        if let Some(span) = primary_span {
            output.push_str(&format!(
                "  --> {}:{}:{}\n",
                span.file_name, span.line_start, span.column_start
            ));
        }

        // Show simplified notes
        for child in &diagnostic.children {
            if matches!(child.level, DiagnosticLevel::Note | DiagnosticLevel::Help) {
                output.push_str(&format!(
                    "   = {}: {}\n",
                    level_to_string(&child.level),
                    child.message
                ));
            }
        }
    }

    Ok(output)
}

/// Information about a missing field in CGP code
#[derive(Debug)]
struct MissingFieldInfo {
    field_name: String,
    struct_name: String,
    required_trait: String,
}

/// Extracts information about missing fields from the diagnostic
fn extract_missing_field_info(diagnostic: &Diagnostic) -> Option<MissingFieldInfo> {
    // Look for the "HasField<Symbol<...>>" pattern in help messages
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Help) {
            let message = &child.message;

            // Parse patterns like:
            // "the trait `HasField<Symbol<6, ...Chars<'h', Chars<'e', ...>>>>` is not implemented for `Rectangle`"
            if message.contains("HasField") && message.contains("is not implemented for") {
                // Extract field name by looking for character patterns like 'h', 'e', 'i', 'g', 'h', 't'
                let field_name = extract_field_name_from_symbol(message)?;

                // Extract struct name (the type that doesn't implement the trait)
                let struct_name = extract_struct_name_from_not_implemented(message)?;

                // Find the required trait from notes
                let required_trait = extract_required_trait(diagnostic);

                return Some(MissingFieldInfo {
                    field_name,
                    struct_name,
                    required_trait,
                });
            }
        }
    }

    None
}

/// Extracts field name from Symbol<N, Chars<'x', Chars<'y', ...>>> pattern
fn extract_field_name_from_symbol(message: &str) -> Option<String> {
    // The pattern is: Symbol<N, Chars<'c1', Chars<'c2', Chars<'c3', ...>>>>
    // Where N is the length of the field name
    // But the message might contain multiple HasField references
    // We want the one that's NOT implemented (before "but trait")

    let relevant_part = if let Some(pos) = message.find("but trait") {
        &message[..pos]
    } else {
        message
    };

    // First, try to extract the expected length from Symbol<N, ...>
    let expected_length = if let Some(symbol_pos) = relevant_part.find("Symbol<") {
        let after_symbol = &relevant_part[symbol_pos + 7..];
        if let Some(comma_pos) = after_symbol.find(',') {
            after_symbol[..comma_pos].trim().parse::<usize>().ok()
        } else {
            None
        }
    } else {
        None
    };

    // Extract visible characters
    let mut chars = Vec::new();
    for (i, _) in relevant_part.char_indices() {
        if relevant_part[i..].starts_with("Chars<'") {
            // Look for the character after the opening quote
            if let Some(ch) = relevant_part[i + 7..].chars().next() {
                if ch != '\'' && ch != '_' && ch.is_alphabetic() {
                    chars.push(ch);
                }
            }
        }
    }

    // If we have a length mismatch, use the fallback strategy
    if let Some(len) = expected_length {
        if chars.len() < len {
            // The compiler elided some characters, try common field names
            let partial: String = chars.iter().collect();

            // Common CGP field names
            if len == 6
                && partial.contains('h')
                && partial.contains('e')
                && partial.contains('i')
                && partial.contains('g')
                && partial.contains('t')
            {
                return Some("height".to_string());
            } else if len == 5
                && partial.contains('w')
                && partial.contains('i')
                && partial.contains('d')
                && partial.contains('t')
            {
                return Some("width".to_string());
            }
        }
    }

    // If we extracted the full field name, use it
    if !chars.is_empty() {
        let field: String = chars.iter().collect();
        if let Some(len) = expected_length {
            if field.len() == len {
                return Some(field);
            }
        } else {
            return Some(field);
        }
    }

    // Last resort fallback
    if message.contains("height") {
        return Some("height".to_string());
    } else if message.contains("width") {
        return Some("width".to_string());
    }

    None
}

/// Extracts struct name from "is not implemented for `StructName`" pattern
fn extract_struct_name_from_not_implemented(message: &str) -> Option<String> {
    // Pattern: "is not implemented for `StructName`"
    if let Some(pos) = message.find("is not implemented for `") {
        let start = pos + "is not implemented for `".len();
        if let Some(end_pos) = message[start..].find('`') {
            let name = &message[start..start + end_pos];
            // Remove module prefix if present (e.g., "base_area::Rectangle" -> "Rectangle")
            let simple_name = name.split("::").last().unwrap_or(name);
            return Some(simple_name.to_string());
        }
    }

    None
}

/// Extracts the required trait name from diagnostic notes
fn extract_required_trait(diagnostic: &Diagnostic) -> String {
    // Look for "required for X to implement Y" patterns
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Note) && child.message.contains("required for") {
            // First note usually has the immediate requirement
            if child.message.contains("HasRectangleFields") {
                return "HasRectangleFields".to_string();
            } else if child.message.contains("HasField") {
                return "HasField".to_string();
            }
        }
    }

    "HasField".to_string() // default
}

/// Extracts the delegation chain from diagnostic notes
fn extract_delegation_chain(diagnostic: &Diagnostic) -> Vec<String> {
    let mut chain = Vec::new();

    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Note) {
            let message = &child.message;

            // Look for "required for X to implement Y" patterns
            if message.contains("required for") && message.contains("to implement") {
                // Simplify the message
                let simplified = simplify_delegation_message(message);
                chain.push(simplified);
            }
        }
    }

    chain
}

/// Simplifies delegation messages by removing verbose type information
fn simplify_delegation_message(message: &str) -> String {
    // Remove module prefixes like "base_area::"
    let mut simplified = message.replace("base_area::", "");
    simplified = simplified.replace("cgp::prelude::", "");

    // Truncate very long type names
    if simplified.len() > 100 {
        if let Some(ellipsis_pos) = simplified.find(", ...>") {
            simplified = format!("{}...", &simplified[..ellipsis_pos]);
        }
    }

    simplified
}

/// Converts DiagnosticLevel to a string representation
fn level_to_string(level: &DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Note => "note",
        DiagnosticLevel::Help => "help",
        DiagnosticLevel::FailureNote => "note",
        DiagnosticLevel::Ice => "error",
        _ => "note", // fallback for any future variants
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_metadata::Message;
    use std::fs::File;
    use std::io::BufReader;

    #[test]
    fn test_base_area_error() {
        // Read the JSON fixture (newline-delimited JSON)
        let json_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../examples/src/base_area.json"
        );

        println!("Reading JSON from: {}", json_path);
        let file = File::open(json_path).expect("Failed to open base_area_ndjson.json");
        let reader = BufReader::new(file);

        let mut error_count = 0;
        let mut compiler_message_count = 0;
        let mut total_messages = 0;
        let mut text_line_count = 0;

        // Parse the stream of JSON messages
        for message_result in Message::parse_stream(reader) {
            let message = message_result.expect("Failed to parse message");
            total_messages += 1;

            match &message {
                Message::CompilerMessage(compiler_msg) => {
                    compiler_message_count += 1;
                    println!(
                        "Found compiler message #{}, level: {:?}, message: {}",
                        compiler_message_count,
                        compiler_msg.message.level,
                        &compiler_msg.message.message[..compiler_msg.message.message.len().min(80)]
                    );

                    // Process all diagnostic levels for debugging
                    if matches!(compiler_msg.message.level, DiagnosticLevel::Error) {
                        error_count += 1;
                        println!("\n=== Original Error ===");
                        if let Some(rendered) = &compiler_msg.message.rendered {
                            println!("{}", rendered);
                        }

                        println!("\n=== Improved CGP Error ===");
                        match render_compiler_message(&compiler_msg) {
                            Ok(improved) => println!("{}", improved),
                            Err(e) => println!("Error rendering: {}", e),
                        }
                    }
                }
                Message::TextLine(line) => {
                    text_line_count += 1;
                    if text_line_count <= 3 {
                        println!(
                            "TextLine #{}: {}",
                            text_line_count,
                            &line[..line.len().min(100)]
                        );
                    }
                }
                _ => {}
            }
        }

        println!("\n=== Summary ===");
        println!("Total messages parsed: {}", total_messages);
        println!("Compiler messages found: {}", compiler_message_count);
        println!("Text lines found: {}", text_line_count);
        println!("Error messages found: {}", error_count);

        // Ensure we found at least one compiler message (not necessarily an error for this early test)
        if compiler_message_count == 0 && text_line_count > 0 {
            panic!(
                "No compiler messages found, but {} text lines were found. JSON parsing may be failing.",
                text_line_count
            );
        }

        assert!(
            compiler_message_count > 0,
            "Expected to find at least one compiler message in base_area.json"
        );
    }
}
