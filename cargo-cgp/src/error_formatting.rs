/// Module for formatting improved error messages from diagnostic entries
/// This implements the approach described in Chapters 10-11 of the report
use crate::cgp_patterns::{derive_provider_trait_name, strip_module_prefixes};
use crate::diagnostic_db::DiagnosticEntry;
use crate::root_cause::{deduplicate_delegation_notes, deduplicate_provider_relationships};

/// Formats a diagnostic entry as an improved CGP error message
pub fn format_error_message(entry: &DiagnosticEntry) -> String {
    let mut output = String::new();

    // Build the error header
    if let Some(code) = &entry.error_code {
        output.push_str(&format!("error[{}]: ", code));
    } else {
        output.push_str("error: ");
    }

    // Format based on what kind of error this is
    if let Some(field_info) = &entry.field_info {
        // This is a missing field error - the most common CGP error
        format_missing_field_error(&mut output, entry, field_info);
    } else {
        // Fallback to a generic CGP error format
        format_generic_cgp_error(&mut output, entry);
    }

    output
}

/// Formats a missing field error with CGP-aware messaging
fn format_missing_field_error(
    output: &mut String,
    entry: &DiagnosticEntry,
    field_info: &crate::cgp_patterns::FieldInfo,
) {
    // Header message
    if field_info.is_complete {
        output.push_str(&format!(
            "missing field `{}` required by CGP component\n",
            field_info.field_name
        ));
    } else {
        output.push_str(&format!(
            "missing field `{}` (possibly incomplete) required by CGP component\n",
            field_info.field_name
        ));
    }

    // Show source location
    if let Some(span) = &entry.primary_span {
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
                let spaces = " ".repeat(span.column_start.saturating_sub(1));
                let carets = "^".repeat(span.column_end.saturating_sub(span.column_start).max(1));
                let label = span.label.as_deref().unwrap_or("");
                output.push_str(&format!("     | {}{} {}\n", spaces, carets, label));
            }
        }

        output.push_str("   |\n");
    }

    // Help message
    if entry.has_other_hasfield_impls {
        output.push_str(&format!(
            "   = help: struct `{}` is missing the field `{}`\n",
            field_info.target_type, field_info.field_name
        ));
    } else {
        output.push_str(&format!(
            "   = help: struct `{}` is either missing the field `{}` or needs `#[derive(HasField)]`\n",
            field_info.target_type, field_info.field_name
        ));
    }

    // Note about which trait requires this field
    if let Some(consumer_trait) = &entry.consumer_trait {
        output.push_str(&format!(
            "   = note: this field is required by the trait bound `{}`\n",
            consumer_trait
        ));
    } else {
        // Fall back to a generic message
        output.push_str("   = note: this field is required by a CGP trait bound\n");
    }

    // Show simplified delegation chain
    if !entry.delegation_notes.is_empty() {
        output.push_str("   = note: delegation chain:\n");
        let simplified_notes = simplify_delegation_chain(entry);
        for note in simplified_notes {
            output.push_str(&format!("           - {}\n", note));
        }
    }

    // Suggest fixes
    if entry.has_other_hasfield_impls {
        output.push_str(&format!(
            "   = help: add `pub {}: <type>` to the `{}` struct definition\n",
            field_info.field_name, field_info.target_type
        ));
    } else {
        output.push_str(&format!(
            "   = help: add `pub {}: <type>` to the `{}` struct definition or add `#[derive(HasField)]` if missing\n",
            field_info.field_name, field_info.target_type
        ));
    }
}

/// Formats a generic CGP error (when we don't have specific field info)
fn format_generic_cgp_error(output: &mut String, entry: &DiagnosticEntry) {
    // Use the original error message
    output.push_str(&entry.message);
    output.push('\n');

    // Show source location
    if let Some(span) = &entry.primary_span {
        output.push_str(&format!(
            "  --> {}:{}:{}\n",
            span.file_name, span.line_start, span.column_start
        ));
        output.push_str("   |\n");

        // Show source lines
        for (i, text_line) in span.text.iter().enumerate() {
            let line_num = span.line_start + i;
            output.push_str(&format!("{:4} | {}\n", line_num, text_line.text));
        }

        output.push_str("   |\n");
    }

    // Show simplified notes
    if !entry.delegation_notes.is_empty() {
        output.push_str("   = note: delegation chain:\n");
        let simplified_notes = simplify_delegation_chain(entry);
        for note in simplified_notes {
            output.push_str(&format!("           - {}\n", note));
        }
    }
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
