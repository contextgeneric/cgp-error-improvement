use miette::{
    GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceOffset, SourceSpan,
};

use crate::cgp_diagnostic::CgpDiagnostic;
use crate::cgp_patterns::{
    ProviderRelationship, derive_provider_trait_name, strip_module_prefixes,
};
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

    // Build help message with clear sections
    let mut help_sections = Vec::new();

    // Section 1: Field name warnings
    if field_info.has_unknown_chars {
        help_sections.push(format!(
            "note: some characters in the field name are hidden by the compiler and shown as '\u{FFFD}'"
        ));
    }

    // Section 2: What's missing
    if entry.has_other_hasfield_impls {
        help_sections.push(format!(
            "The struct `{}` is missing the required field `{}`.",
            field_info.target_type, formatted_field_name
        ));
    } else {
        help_sections.push(format!(
            "The struct `{}` is either missing the field `{}` or is missing `#[derive(HasField)]`.",
            field_info.target_type, formatted_field_name
        ));
    }

    // Section 3: Component context (only if we have a valid component name)
    if let Some(component_info) = &entry.component_info {
        // Strip module prefixes and validate the component name
        let clean_component = strip_module_prefixes(&component_info.component_type);

        // Only show if it looks like a valid component name (not a malformed extraction)
        if !clean_component.contains("IsProviderFor<")
            && !clean_component.contains("CanUseComponent<")
        {
            help_sections.push(format!(
                "This field is required by the component `{}`.",
                clean_component
            ));
        }
    }

    // Section 4: Check trait context (if available)
    if let Some(check_trait) = &entry.check_trait {
        help_sections.push(format!(
            "The check trait `{}` verifies that all required components are available.",
            check_trait
        ));
    }

    // Section 5: Delegation chain
    if !entry.delegation_notes.is_empty() {
        help_sections.push(String::new()); // Blank line
        help_sections.push("Dependency chain:".to_string());
        let delegation_lines = format_delegation_chain(entry);
        for line in delegation_lines {
            help_sections.push(format!("  {}", line));
        }
    }

    // Section 6: How to fix
    help_sections.push(String::new()); // Blank line
    help_sections.push("To fix this error:".to_string());
    if entry.has_other_hasfield_impls {
        help_sections.push(format!(
            "  - Add the field `{}` to the `{}` struct",
            formatted_field_name, field_info.target_type
        ));
    } else {
        help_sections.push(format!(
            "  - Add `#[derive(HasField)]` to the `{}` struct, if missing",
            field_info.target_type
        ));
        help_sections.push(format!(
            "  - Ensure the field `{}` exists in the struct",
            formatted_field_name
        ));
    }

    let help = Some(help_sections.join("\n"));

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
    let mut help_sections = Vec::new();

    if !entry.delegation_notes.is_empty() {
        help_sections.push("Dependency chain:".to_string());
        let delegation_lines = format_delegation_chain(entry);
        for line in delegation_lines {
            help_sections.push(format!("  {}", line));
        }
    }

    let help = if help_sections.is_empty() {
        None
    } else {
        Some(help_sections.join("\n"))
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

    // Try to read the actual source file to get proper content and offsets
    // The file_name might be absolute or relative
    let file_result = std::fs::read_to_string(&span.file_name).or_else(|_| {
        // If the path is relative, try from the workspace root
        // Look for common workspace patterns
        if let Ok(current_dir) = std::env::current_dir() {
            // Try current directory first
            let candidate1 = current_dir.join(&span.file_name);
            if let Ok(content) = std::fs::read_to_string(&candidate1) {
                return Ok(content);
            }

            // Try parent directory (in case we're in a subdirectory)
            if let Some(parent) = current_dir.parent() {
                let candidate2 = parent.join(&span.file_name);
                if let Ok(content) = std::fs::read_to_string(&candidate2) {
                    return Ok(content);
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find source file",
        ))
    });

    match file_result {
        Ok(file_content) => {
            // Use the actual file content
            let source_code = NamedSource::new(&span.file_name, file_content.clone());

            // Calculate byte offset in the actual file
            // Count bytes up to the line, then add column offset
            let lines: Vec<&str> = file_content.lines().collect();

            let mut byte_offset = 0;

            // Add bytes for all lines before the target line (1-indexed)
            for (line_idx, line) in lines.iter().enumerate() {
                if line_idx + 1 < span.line_start {
                    byte_offset += line.len() + 1; // +1 for newline
                } else {
                    break;
                }
            }

            // Add column offset (1-indexed, so subtract 1)
            byte_offset += span.column_start.saturating_sub(1);

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
        Err(_) => {
            // Fallback: reconstruct from span text
            let source_text = span
                .text
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            if source_text.is_empty() {
                // If we have no source text at all, just return nothing
                return (None, vec![]);
            }

            let source_code = NamedSource::new(&span.file_name, source_text);

            // For fallback, use simple column offset
            let byte_offset = span.column_start.saturating_sub(1);
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
    }
}

/// Formats the delegation chain with better structure and CGP-aware terminology
fn format_delegation_chain(entry: &DiagnosticEntry) -> Vec<String> {
    // Detect inner providers BEFORE deduplication
    let all_inner_providers: Vec<String> = detect_inner_providers(&entry.provider_relationships);

    // First deduplicate the provider relationships to remove nested redundancies
    let deduped_relationships = deduplicate_provider_relationships(&entry.provider_relationships);

    // Build a set of provider types we should keep
    let kept_provider_types: std::collections::HashSet<String> = deduped_relationships
        .iter()
        .map(|r| r.provider_type.clone())
        .collect();

    // Deduplicate notes first
    let deduped_notes = deduplicate_delegation_notes(&entry.delegation_notes);

    let mut formatted = Vec::new();

    // If we have inner providers and field errors, add a hint about the root cause
    // We check this AFTER deduplication to see which outer providers remain
    if !all_inner_providers.is_empty() && entry.field_info.is_some() {
        let outer_providers: Vec<_> = deduped_relationships
            .iter()
            .filter(|r| {
                !all_inner_providers
                    .iter()
                    .any(|inner| inner == &r.provider_type)
            })
            .collect();

        if !outer_providers.is_empty() && !all_inner_providers.is_empty() {
            formatted.push(format!(
                "→ The error in `{}` is caused by the inner provider `{}`",
                outer_providers[0].provider_type, all_inner_providers[0]
            ));
        }
    }

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

        let formatted_note = format_delegation_note(&note, entry);
        formatted.push(format!("→ {}", formatted_note));
    }

    formatted
}

/// Detects inner providers in a list of provider relationships
/// Returns the list of inner provider types (those that appear as type parameters in other providers)
fn detect_inner_providers(relationships: &[ProviderRelationship]) -> Vec<String> {
    let mut inner_providers = Vec::new();

    for rel in relationships {
        // Check if this provider appears as a type parameter in any other provider
        for other in relationships {
            if rel.provider_type != other.provider_type {
                if is_contained_type_parameter(&rel.provider_type, &other.provider_type) {
                    if !inner_providers.contains(&rel.provider_type) {
                        inner_providers.push(rel.provider_type.clone());
                    }
                }
            }
        }
    }

    inner_providers
}

/// Checks if inner_type appears as a type parameter within outer_type
/// For example, "RectangleArea" is contained in "ScaledArea<RectangleArea>"
fn is_contained_type_parameter(inner_type: &str, outer_type: &str) -> bool {
    // Check various patterns where inner could appear in outer
    let patterns = [
        format!("<{}>", inner_type),
        format!("<{},", inner_type),
        format!(", {}>", inner_type),
        format!(", {},", inner_type),
        format!("< {}", inner_type), // with spaces
        format!("{} >", inner_type),
    ];

    patterns.iter().any(|pattern| outer_type.contains(pattern))
}

/// Simplifies a single delegation note
fn format_delegation_note(note: &str, _entry: &DiagnosticEntry) -> String {
    let mut result = note.to_string();

    // Remove module prefixes
    result = strip_module_prefixes(&result);

    // Replace IsProviderFor with user-friendly "provider trait" terminology
    result = replace_is_provider_for(&result);

    // Replace CanUseComponent with simpler terminology
    result = replace_can_use_component(&result);

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

/// Replaces `CanUseComponent<Component>` with simpler terminology
fn replace_can_use_component(message: &str) -> String {
    if !message.contains("CanUseComponent") {
        return message.to_string();
    }

    // Find the CanUseComponent pattern
    if let Some(start) = message.find("CanUseComponent<") {
        let after_start = start + "CanUseComponent<".len();

        // Find the end of the generic type
        let end_pos = find_matching_bracket(after_start, message).unwrap_or(message.len());

        let component_name = message[after_start..end_pos].trim();

        // Build replacement - just explain it's checking component availability
        let replacement = format!("use component `{}`", component_name);

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

    match handler.render_report(&mut output, diagnostic) {
        Ok(_) => output,
        Err(_) => {
            // Fallback to simple display if rendering fails
            format!("error: {}", diagnostic.message)
        }
    }
}

/// Renders a CGP diagnostic to a plain text string (no colors)
pub fn render_diagnostic_plain(diagnostic: &CgpDiagnostic) -> String {
    // Use the narratable handler which produces plain text
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none());
    let mut output = String::new();

    match handler.render_report(&mut output, diagnostic) {
        Ok(_) => output,
        Err(_) => {
            // Fallback to simple display if rendering fails
            format!("error: {}", diagnostic.message)
        }
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
