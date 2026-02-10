use miette::{
    GraphicalReportHandler, GraphicalTheme, LabeledSpan, MietteHandler, NamedSource, NarratableReportHandler, SourceOffset, SourceSpan
};

use crate::cgp_diagnostic::CgpDiagnostic;
use crate::cgp_patterns::{derive_provider_trait_name, strip_module_prefixes};
use crate::diagnostic_db::DiagnosticEntry;
use crate::root_cause::{deduplicate_delegation_notes, deduplicate_provider_relationships};

/// Checks if a field name contains non-basic identifier characters
/// Basic identifier characters are: a-z, A-Z, 0-9, underscore, hyphen, and the replacement character
fn has_non_basic_identifier_chars(field_name: &str) -> bool {
    field_name
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '\u{FFFD}')
}

/// Formats a field name for display, escaping it like a Rust string if it contains special characters
fn format_field_name(field_name: &str) -> String {
    if has_non_basic_identifier_chars(field_name) {
        // Escape like a Rust string
        format!("\"{}\"", field_name.escape_default())
    } else {
        // Display as-is
        field_name.to_string()
    }
}

/// Formats a diagnostic entry as an improved CGP error message
pub fn format_error_message(entry: &DiagnosticEntry) -> Option<CgpDiagnostic> {
    // Format based on what kind of error this is
    if let Some(field_info) = &entry.field_info {
        // This is a missing field error - the most common CGP error
        format_missing_field_error(entry, field_info)
    } else {
        // Fallback to a generic CGP error format
        format_generic_cgp_error(entry)
    }
}

/// Formats a missing field error with CGP-aware messaging
fn format_missing_field_error(
    entry: &DiagnosticEntry,
    field_info: &crate::cgp_patterns::FieldInfo,
) -> Option<CgpDiagnostic> {
    let formatted_field_name = format_field_name(&field_info.field_name);

    // Build the main error message
    let message = if field_info.is_complete {
        format!(
            "missing field `{}` required by CGP component",
            formatted_field_name
        )
    } else {
        format!(
            "missing field `{}` (possibly incomplete) required by CGP component",
            formatted_field_name
        )
    };

    // Build help message
    let mut help_parts = Vec::new();

    if field_info.has_unknown_chars {
        help_parts.push(format!(
            "note: some characters in the field name are hidden by the compiler and shown as '\u{FFFD}'"
        ));
    }

    if entry.has_other_hasfield_impls {
        help_parts.push(format!(
            "the struct `{}` is missing the required field `{}`",
            field_info.target_type, formatted_field_name
        ));
        help_parts.push(format!(
            "ensure a field `{}` of the appropriate type is present in the `{}` struct",
            formatted_field_name, field_info.target_type
        ));
    } else {
        help_parts.push(format!(
            "the struct `{}` is either missing the field `{}` or is missing `#[derive(HasField)]`",
            field_info.target_type, formatted_field_name
        ));
        help_parts.push(format!(
            "ensure a field `{}` of the appropriate type is present in the `{}` struct, or add `#[derive(HasField)]` if the struct is missing the derive",
            formatted_field_name, field_info.target_type
        ));
    }

    // Add note about which trait requires this field
    if let Some(consumer_trait) = &entry.consumer_trait {
        help_parts.push(format!(
            "note: this field is required by the trait bound `{}`",
            consumer_trait
        ));
    } else {
        help_parts.push("note: this field is required by a CGP trait bound".to_string());
    }

    // Add delegation chain
    if !entry.delegation_notes.is_empty() {
        help_parts.push("note: delegation chain:".to_string());
        let simplified_notes = simplify_delegation_chain(entry);
        for note in simplified_notes {
            help_parts.push(format!("  - {}", note));
        }
    }

    let help = if help_parts.is_empty() {
        None
    } else {
        Some(help_parts.join("\n"))
    };

    // Build source code and labels
    let (source_code, labels) = build_source_and_labels(entry);

    Some(CgpDiagnostic {
        message,
        code: entry.error_code.clone(),
        help,
        source_code,
        labels,
    })
}

/// Formats a generic CGP error (when we don't have specific field info)
fn format_generic_cgp_error(entry: &DiagnosticEntry) -> Option<CgpDiagnostic> {
    let message = entry.message.clone();

    // Build help with simplified notes
    let mut help_parts = Vec::new();

    if !entry.delegation_notes.is_empty() {
        help_parts.push("note: delegation chain:".to_string());
        let simplified_notes = simplify_delegation_chain(entry);
        for note in simplified_notes {
            help_parts.push(format!("  - {}", note));
        }
    }

    let help = if help_parts.is_empty() {
        None
    } else {
        Some(help_parts.join("\n"))
    };

    // Build source code and labels
    let (source_code, labels) = build_source_and_labels(entry);

    Some(CgpDiagnostic {
        message,
        code: entry.error_code.clone(),
        help,
        source_code,
        labels,
    })
}

/// Builds source code and labeled spans from diagnostic entry
fn build_source_and_labels(
    entry: &DiagnosticEntry,
) -> (Option<NamedSource<String>>, Vec<LabeledSpan>) {
    let span = match &entry.primary_span {
        Some(s) => s,
        None => return (None, vec![]),
    };

    // Reconstruct the source code from the span's text lines
    let source_text = span
        .text
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Create named source
    let source_code = NamedSource::new(&span.file_name, source_text);

    // Create a labeled span for the primary location
    // Calculate the byte offset within our reconstructed source
    let byte_offset = if span.line_start > 1 {
        // If the span doesn't start at line 1, we need to calculate offset
        // For simplicity, we'll use column offset from start of first line
        span.column_start.saturating_sub(1)
    } else {
        span.column_start.saturating_sub(1)
    };

    let span_length = span.column_end.saturating_sub(span.column_start).max(1);

    let label_text = span
        .label
        .clone()
        .unwrap_or_else(|| "error occurs here".to_string());

    let labeled_span = LabeledSpan::new_with_span(
        Some(label_text),
        SourceSpan::new(SourceOffset::from(byte_offset), span_length),
    );

    (Some(source_code), vec![labeled_span])
}

/// Simplifies the delegation chain by removing redundancy and using CGP-aware terminology
fn simplify_delegation_chain(entry: &DiagnosticEntry) -> Vec<String> {
    // First deduplicate the provider relationships to remove nested redundancies
    let deduped_relationships = deduplicate_provider_relationships(&entry.provider_relationships);

    // Build a set of provider types we should keep
    let kept_provider_types: std::collections::HashSet<String> = deduped_relationships
        .iter()
        .map(|r| r.provider_type.clone())
        .collect();

    // Deduplicate notes first
    let deduped_notes = deduplicate_delegation_notes(&entry.delegation_notes);

    let mut simplified = Vec::new();

    for note in deduped_notes {
        // Parse provider info from the note to check if it should be kept
        let should_keep = if let Some(provider_info) =
            crate::cgp_patterns::extract_provider_relationship(&note)
        {
            // Keep this note only if its provider type is in the kept set, OR if we have no provider info
            kept_provider_types.is_empty()
                || kept_provider_types.contains(&provider_info.provider_type)
        } else {
            // Not a provider relationship note, always keep it
            true
        };

        if !should_keep {
            // Skip this note as it's redundant
            continue;
        }

        let simplified_note = simplify_delegation_note(&note, entry);
        simplified.push(simplified_note);
    }

    simplified
}

/// Simplifies a single delegation note
fn simplify_delegation_note(note: &str, entry: &DiagnosticEntry) -> String {
    let mut result = note.to_string();

    // Remove module prefixes
    result = strip_module_prefixes(&result);

    // Replace IsProviderFor with user-friendly "provider trait" terminology
    result = replace_is_provider_for(&result);

    // Replace CanUseComponent with user-friendly "consumer trait" terminology
    result = replace_can_use_component(&result, entry);

    // Truncate overly long type names
    if result.len() > 150 {
        if let Some(ellipsis_pos) = result.find(", ...>") {
            result = format!("{}...", &result[..ellipsis_pos]);
        }
    }

    result
}

/// Replaces `IsProviderFor<Component, Context>` with "the provider trait `ProviderTrait`"
fn replace_is_provider_for(message: &str) -> String {
    if !message.contains("IsProviderFor") {
        return message.to_string();
    }

    // Find the IsProviderFor pattern
    if let Some(start) = message.find("IsProviderFor<") {
        let after_start = start + "IsProviderFor<".len();

        // Extract component name (up to the first comma)
        if let Some(comma_pos) = find_top_level_comma(after_start, message) {
            let component_name = message[after_start..comma_pos].trim();

            // Derive provider trait name
            let provider_trait_name = derive_provider_trait_name(component_name)
                .unwrap_or_else(|| format!("the provider trait for `{}`", component_name));

            // Find the end of IsProviderFor<...>
            let end_pos = find_matching_bracket(after_start, message).unwrap_or(message.len());

            // Build replacement
            let before = &message[..start];
            let after = &message[end_pos..];

            // Handle backticks
            let has_opening_backtick = before.ends_with('`');
            let has_closing_backtick = after.starts_with('`');

            if has_opening_backtick && has_closing_backtick {
                return format!(
                    "{}the provider trait `{}`{}",
                    &before[..before.len() - 1],
                    provider_trait_name,
                    &after[1..]
                );
            } else {
                return format!(
                    "{}the provider trait `{}`{}",
                    before, provider_trait_name, after
                );
            }
        }
    }

    message.to_string()
}

/// Replaces `CanUseComponent<Component>` with user-friendly "the consumer trait for `Component`"
fn replace_can_use_component(message: &str, entry: &DiagnosticEntry) -> String {
    if !message.contains("CanUseComponent") {
        return message.to_string();
    }

    // Find the CanUseComponent pattern
    if let Some(start) = message.find("CanUseComponent<") {
        let after_start = start + "CanUseComponent<".len();

        // Find the end of the generic type
        let end_pos = find_matching_bracket(after_start, message).unwrap_or(message.len());

        let component_name = message[after_start..end_pos].trim();

        // Build replacement using consumer trait if available
        let replacement = if let Some(consumer_trait) = &entry.consumer_trait {
            format!("the consumer trait `{}`", consumer_trait)
        } else {
            format!("the consumer trait for `{}`", component_name)
        };

        // Handle backticks
        let before = &message[..start];
        let after = &message[end_pos + 1..];

        let has_opening_backtick = before.ends_with('`');
        let has_closing_backtick = after.starts_with('`');

        if has_opening_backtick && has_closing_backtick {
            return format!(
                "{}{}{}",
                &before[..before.len() - 1],
                replacement,
                &after[1..]
            );
        } else {
            return format!("{}{}{}", before, replacement, after);
        }
    }

    message.to_string()
}

/// Finds the position of a comma at the top level of generic nesting
fn find_top_level_comma(start_pos: usize, text: &str) -> Option<usize> {
    let mut depth = 0;

    for (i, ch) in text[start_pos..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => return Some(start_pos + i),
            _ => {}
        }
    }

    None
}

/// Finds the position of the matching closing bracket
fn find_matching_bracket(start_pos: usize, text: &str) -> Option<usize> {
    let mut depth = 1;

    for (i, ch) in text[start_pos..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start_pos + i + 1);
                }
            }
            _ => {}
        }
    }

    None
}

/// Renders a CGP diagnostic to a string using the graphical (colorful) handler
pub fn render_diagnostic_graphical(diagnostic: &CgpDiagnostic) -> String {
    let handler = GraphicalReportHandler::new();
    let mut output = String::new();

    if handler.render_report(&mut output, diagnostic).is_ok() {
        output
    } else {
        // Fallback to simple display if rendering fails
        format!("error: {}", diagnostic.message)
    }
}

/// Renders a CGP diagnostic to a plain text string (no colors)
pub fn render_diagnostic_plain(diagnostic: &CgpDiagnostic) -> String {
    // Use the narratable handler which produces plain text
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none());
    let mut output = String::new();

    if handler.render_report(&mut output, diagnostic).is_ok() {
        output
    } else {
        // Fallback to simple display if rendering fails
        format!("error: {}", diagnostic.message)
    }
}

/// Detects if we're running in a terminal that supports colors
pub fn is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_is_provider_for() {
        let input =
            "required for `Foo` to implement `IsProviderFor<AreaCalculatorComponent, Context>`";
        let output = replace_is_provider_for(input);
        assert!(output.contains("provider trait `AreaCalculator`"));
        assert!(!output.contains("IsProviderFor"));
    }

    #[test]
    fn test_find_top_level_comma() {
        let text = "IsProviderFor<Foo<A, B>, Bar>";
        let start = "IsProviderFor<".len();
        if let Some(pos) = find_top_level_comma(start, text) {
            assert_eq!(&text[start..pos], "Foo<A, B>");
        } else {
            panic!("Should find comma");
        }
    }
}
