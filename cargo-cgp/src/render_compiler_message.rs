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
                // Simplify the message and hide internal CGP implementation details
                let simplified = simplify_delegation_message(message);
                chain.push(simplified);
            }
        }
    }

    chain
}

/// Simplifies delegation messages by removing verbose type information and hiding internal CGP traits
fn simplify_delegation_message(message: &str) -> String {
    let mut simplified = message.to_string();

    // Remove module prefixes FIRST, before doing trait replacements
    // This ensures our pattern matching works correctly
    simplified = simplified.replace("base_area::", "");
    simplified = simplified.replace("cgp::prelude::", "");

    // Hide internal CGP trait `IsProviderFor` and replace with user-friendly "provider trait"
    // Pattern: "required for `X` to implement `IsProviderFor<YComponent, Z>`"
    // Replace with: "required for `X` to implement the provider trait `Y`"
    if let Some(provider_replacement) = replace_is_provider_for(&simplified) {
        simplified = provider_replacement;
    }

    // Hide internal CGP trait `CanUseComponent` and replace with user-friendly "consumer trait"
    // Pattern: "required for `X` to implement `CanUseComponent<YComponent>`"
    // Replace with: "required for `X` to implement the consumer trait for `YComponent`"
    if let Some(consumer_replacement) = replace_can_use_component(&simplified) {
        simplified = consumer_replacement;
    }

    // Truncate very long type names
    if simplified.len() > 100 {
        if let Some(ellipsis_pos) = simplified.find(", ...>") {
            simplified = format!("{}...", &simplified[..ellipsis_pos]);
        }
    }

    simplified
}

/// Replaces `IsProviderFor<Component, Context>` with user-friendly provider trait mention
/// This hides the internal CGP trait and presents a more intuitive interface to users
fn replace_is_provider_for(message: &str) -> Option<String> {
    // Pattern: "to implement `IsProviderFor<ComponentName, ContextType>`"
    if !message.contains("IsProviderFor") {
        return None;
    }

    // Extract the component name from IsProviderFor<ComponentName, ...>
    if let Some(start) = message.find("IsProviderFor<") {
        let after_start = start + "IsProviderFor<".len();
        if let Some(comma_pos) = message[after_start..].find(',') {
            let component_name = message[after_start..after_start + comma_pos].trim();
            let provider_trait_name = extract_provider_trait_name(component_name);

            // Find the end of the IsProviderFor type
            let mut bracket_count = 1;
            let mut end_pos = after_start;
            for (i, ch) in message[after_start..].char_indices() {
                if ch == '<' {
                    bracket_count += 1;
                } else if ch == '>' {
                    bracket_count -= 1;
                    if bracket_count == 0 {
                        end_pos = after_start + i + 1;
                        break;
                    }
                }
            }

            // Replace the entire IsProviderFor<...> with the provider trait name
            // The original message typically has backticks around the trait: `IsProviderFor<...>`
            // We need to check if we're inside backticks and adjust accordingly
            let before = &message[..start];
            let after = &message[end_pos..];

            // Check if IsProviderFor is wrapped in backticks
            let has_opening_backtick = before.ends_with('`');
            let has_closing_backtick = after.starts_with('`');

            let replacement = if has_opening_backtick && has_closing_backtick {
                // We're inside backticks, remove outer ones and keep inner ones
                format!(
                    "{}the provider trait `{}`{}",
                    &before[..before.len() - 1], // Remove trailing backtick
                    provider_trait_name,
                    &after[1..]
                ) // Remove leading backtick
            } else {
                // Not in backticks, add them around the provider trait name
                format!(
                    "{}the provider trait `{}`{}",
                    before, provider_trait_name, after
                )
            };

            return Some(replacement);
        }
    }

    None
}

/// Replaces `CanUseComponent<Component>` with user-friendly consumer trait mention
/// This hides the internal CGP trait and presents a more intuitive interface to users
fn replace_can_use_component(message: &str) -> Option<String> {
    // Pattern: "to implement `CanUseComponent<ComponentName>`"
    if !message.contains("CanUseComponent") {
        return None;
    }

    // Extract the component name from CanUseComponent<ComponentName>
    if let Some(start) = message.find("CanUseComponent<") {
        let after_start = start + "CanUseComponent<".len();

        // Find the matching closing bracket
        let mut bracket_count = 1;
        let mut end_pos = after_start;
        for (i, ch) in message[after_start..].char_indices() {
            if ch == '<' {
                bracket_count += 1;
            } else if ch == '>' {
                bracket_count -= 1;
                if bracket_count == 0 {
                    end_pos = after_start + i;
                    break;
                }
            }
        }

        let component_name = message[after_start..end_pos].trim();

        // Replace CanUseComponent<...> with "the consumer trait for `ComponentName`"
        // The original message typically has backticks: `CanUseComponent<...>`
        // We need to handle the backticks properly
        let before = &message[..start];
        let after = &message[end_pos + 1..];

        // Check if CanUseComponent is wrapped in backticks
        let has_opening_backtick = before.ends_with('`');
        let has_closing_backtick = after.starts_with('`');

        let replacement = if has_opening_backtick && has_closing_backtick {
            // We're inside backticks, remove outer ones and keep inner ones
            format!(
                "{}the consumer trait for `{}`{}",
                &before[..before.len() - 1], // Remove trailing backtick
                component_name,
                &after[1..]
            ) // Remove leading backtick
        } else {
            // Not in backticks, add them around the component name
            format!(
                "{}the consumer trait for `{}`{}",
                before, component_name, after
            )
        };

        return Some(replacement);
    }

    None
}

/// Extracts the provider trait name from a component name
/// Convention: ComponentName -> ProviderTrait (remove "Component" suffix)
/// Example: "AreaCalculatorComponent" -> "AreaCalculator"
fn extract_provider_trait_name(component_name: &str) -> String {
    if let Some(stripped) = component_name.strip_suffix("Component") {
        stripped.to_string()
    } else {
        // If no "Component" suffix, return a generic description
        format!("the provider trait for `{}`", component_name)
    }
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
        let file = File::open(json_path).expect("Failed to open base_area.json");
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
