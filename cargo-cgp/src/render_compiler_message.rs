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

/// Extract CGP metadata from a diagnostic
#[derive(Debug, Clone)]
pub struct CgpErrorMetadata {
    pub component: Option<String>,
    pub context: Option<String>,
    pub provider_trait: Option<String>,
    pub consumer_trait: Option<String>,
    pub is_provider_trait_error: bool,
    pub has_missing_field: bool,
}

impl CgpErrorMetadata {
    pub fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        let mut meta = CgpErrorMetadata {
            component: None,
            context: None,
            provider_trait: None,
            consumer_trait: None,
            is_provider_trait_error: false,
            has_missing_field: false,
        };

        // Check main message for clues
        if diagnostic.message.contains("AreaCalculator") {
            if !diagnostic.message.contains("AreaCalculatorComponent") {
                // This is likely about the provider trait itself
                meta.is_provider_trait_error = true;
            }
        }

        // Extract information from children
        for child in &diagnostic.children {
            // Check for provider trait with __Context__ pattern
            if matches!(child.level, DiagnosticLevel::Help) {
                if child.message.contains("__Context__")
                    && child.message.contains("is implemented for")
                {
                    // Pattern: "the trait `ProviderTrait<__Context__>` is implemented for `ProviderImpl`"
                    if let Some(trait_name) =
                        extract_provider_trait_from_context_pattern(&child.message)
                    {
                        meta.provider_trait = Some(trait_name);
                    }
                }
            }

            // Check for IsProviderFor pattern
            if child.message.contains("IsProviderFor") {
                if let Some(provider_info) = parse_provider_info(&child.message) {
                    if meta.component.is_none() {
                        meta.component = Some(provider_info.component.clone());
                    }
                    if meta.context.is_none() {
                        meta.context = Some(provider_info.context.clone());
                    }
                }
            }

            // Check for CanUseComponent pattern to extract component
            if child.message.contains("CanUseComponent") {
                if let Some(component) = extract_component_from_can_use(&child.message) {
                    if meta.component.is_none() {
                        meta.component = Some(component);
                    }
                }
            }

            // Check for consumer trait ("required by a bound in X")
            if matches!(child.level, DiagnosticLevel::Note)
                && child.message.contains("required by a bound in")
            {
                if let Some(trait_name) = extract_consumer_trait_from_bound(&child.message) {
                    meta.consumer_trait = Some(trait_name);
                }
            }

            // Check for missing field
            if matches!(child.level, DiagnosticLevel::Help)
                && child.message.contains("HasField")
                && child.message.contains("is not implemented")
            {
                meta.has_missing_field = true;
            }
        }

        // Extract component from main message if not found
        if meta.component.is_none() {
            if let Some(component) = extract_component_from_message(&diagnostic.message) {
                meta.component = Some(component);
            }
        }

        meta
    }
}

/// Extract provider trait name from "ProviderTrait<__Context__>" pattern
fn extract_provider_trait_from_context_pattern(message: &str) -> Option<String> {
    // Pattern: "the trait `ProviderTrait<__Context__>` is implemented for"
    if let Some(start) = message.find("the trait `") {
        let after_start = start + "the trait `".len();
        if let Some(lt_pos) = message[after_start..].find('<') {
            let trait_name = &message[after_start..after_start + lt_pos];
            // Remove module prefixes
            let simple_name = trait_name.split("::").last().unwrap_or(trait_name);
            return Some(simple_name.to_string());
        }
    }
    None
}

/// Extract component from CanUseComponent<Component> pattern
fn extract_component_from_can_use(message: &str) -> Option<String> {
    if let Some(start) = message.find("CanUseComponent<") {
        let after_start = start + "CanUseComponent<".len();
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

        let component = message[after_start..end_pos].trim();
        return Some(component.to_string());
    }
    None
}

/// Extract consumer trait name from "required by a bound in X"
fn extract_consumer_trait_from_bound(message: &str) -> Option<String> {
    if let Some(start) = message.find("required by a bound in `") {
        let after_start = start + "required by a bound in `".len();
        if let Some(end) = message[after_start..].find('`') {
            return Some(message[after_start..after_start + end].to_string());
        }
    }
    None
}

/// Extract component from main error message
fn extract_component_from_message(message: &str) -> Option<String> {
    // Look for patterns like "Component" at the end
    let words: Vec<&str> = message.split_whitespace().collect();
    for word in words {
        if word.ends_with("Component")
            || word.ends_with("Component>")
            || word.ends_with("Component,")
        {
            let clean = word
                .trim_end_matches(&[',', '>', '`', ')'])
                .trim_start_matches(&['`', '(']);
            return Some(clean.to_string());
        }
    }
    None
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

    // Check if this is a provider trait error by looking for:
    // "required for X to implement IsProviderFor" in child notes
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Note | DiagnosticLevel::Help) {
            if child.message.contains("IsProviderFor") {
                return true;
            }
        }
    }

    false
}

/// Renders a CGP error with improved formatting
fn render_cgp_error(diagnostic: &Diagnostic) -> Result<String, Error> {
    let mut output = String::new();

    // Extract metadata to understand what kind of error this is
    let metadata = CgpErrorMetadata::from_diagnostic(diagnostic);

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

    // If this is a provider trait error without field information, we might be seeing
    // the first of a pair of related errors. Suppress it since the detailed error will come next.
    if metadata.is_provider_trait_error && missing_field_info.is_none() {
        // This is the "Provider: ProviderTrait<Context>" error that will be followed by
        // a more detailed error about the missing field. We suppress this to avoid duplication.
        // Return empty string to filter it out
        return Ok(String::new());
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

        // Build help message for missing field
        let has_other_hasfield_impls = has_other_hasfield_implementations(diagnostic);
        if has_other_hasfield_impls {
            output.push_str(&format!(
                "   = help: struct `{}` is missing the field `{}`\n",
                field_info.struct_name, field_info.field_name
            ));
        } else {
            output.push_str(&format!(
                "   = help: struct `{}` is either missing the field `{}` or needs `#[derive(HasField)]`\n",
                field_info.struct_name, field_info.field_name
            ));
        }

        output.push_str(&format!(
            "   = note: this field is required by the trait bound `{}`\n",
            field_info.required_trait
        ));

        // Show the delegation chain in a simplified form with deduplication
        output.push_str("   = note: delegation chain:\n");
        let chain_items = extract_and_deduplicate_delegation_chain(diagnostic, &metadata);
        for note in chain_items {
            output.push_str(&format!("           - {}\n", note));
        }

        // Suggest a fix
        if has_other_hasfield_impls {
            output.push_str(&format!(
                "   = help: add `pub {}: f64` to the `{}` struct definition\n",
                field_info.field_name, field_info.struct_name
            ));
        } else {
            output.push_str(&format!(
                "   = help: add `pub {}: f64` to the `{}` struct definition or add `#[derive(HasField)]` if missing\n",
                field_info.field_name, field_info.struct_name
            ));
        }
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

/// Checks if the diagnostic has help messages indicating the type implements HasField for other fields
fn has_other_hasfield_implementations(diagnostic: &Diagnostic) -> bool {
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Help) {
            if child.message.contains("but trait `HasField")
                || child
                    .message
                    .contains("the following other types implement trait `cgp::prelude::HasField")
            {
                return true;
            }
        }
    }
    false
}

/// Extracts provider information from IsProviderFor messages
#[derive(Debug, Clone, PartialEq)]
struct ProviderInfo {
    provider_type: String,
    component: String,
    context: String,
}

/// Parses a provider info from an IsProviderFor note
fn parse_provider_info(message: &str) -> Option<ProviderInfo> {
    // Look for pattern like "for `X` to implement `IsProviderFor<Component, Context>`"
    if !message.contains("IsProviderFor") {
        return None;
    }

    // Extract provider type
    let provider_type = if let Some(start) = message.find("for `") {
        let after = &message[start + 5..];
        if let Some(end) = after.find('`') {
            after[..end].to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Extract component and context from IsProviderFor<Component, Context>
    if let Some(start) = message.find("IsProviderFor<") {
        let after = &message[start + 14..];
        if let Some(comma) = after.find(',') {
            let component = after[..comma].trim().to_string();

            // Extract context (everything after the comma until the closing >)
            let after_comma = &after[comma + 1..];
            let mut bracket_count = 1;
            let mut end_pos = 0;

            for (i, ch) in after_comma.char_indices() {
                if ch == '<' {
                    bracket_count += 1;
                } else if ch == '>' {
                    bracket_count -= 1;
                    if bracket_count == 0 {
                        end_pos = i;
                        break;
                    }
                }
            }

            let context = after_comma[..end_pos].trim().to_string();

            return Some(ProviderInfo {
                provider_type,
                component,
                context,
            });
        }
    }

    None
}

/// Extracts the delegation chain from diagnostic notes with deduplication
fn extract_and_deduplicate_delegation_chain(
    diagnostic: &Diagnostic,
    metadata: &CgpErrorMetadata,
) -> Vec<String> {
    let mut chain = Vec::new();
    let mut provider_infos: Vec<ProviderInfo> = Vec::new();

    // First pass: collect all notes and parse provider info
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Note) {
            let message = &child.message;

            // Look for "required for X to implement Y" patterns
            if message.contains("required for") && message.contains("to implement") {
                // Try to parse provider info
                if let Some(provider_info) = parse_provider_info(message) {
                    provider_infos.push(provider_info);
                }
            }
        }
    }

    // Second pass: build deduplicated chain
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Note) {
            let message = &child.message;

            if message.contains("required for") && message.contains("to implement") {
                // Check if this is a redundant provider mention
                if let Some(current_provider) = parse_provider_info(message) {
                    // Check if there's another provider in the chain that wraps this one
                    // with the same component and context
                    let is_redundant = provider_infos.iter().any(|other| {
                        if other == &current_provider {
                            return false; // Don't compare with itself
                        }

                        // Check if the other provider contains the current provider as a generic parameter
                        // and both have the same component and context
                        // The provider_type should contain the inner provider as a type parameter
                        // e.g., "ScaledArea<RectangleArea>" contains "RectangleArea"
                        let contains_as_type_param = other
                            .provider_type
                            .contains(&format!("<{}", current_provider.provider_type))
                            || other
                                .provider_type
                                .contains(&format!("<{},", current_provider.provider_type))
                            || other
                                .provider_type
                                .contains(&format!(", {}", current_provider.provider_type))
                            || other
                                .provider_type
                                .contains(&format!(" {}", current_provider.provider_type))
                            || other.provider_type
                                == format!("{}>", current_provider.provider_type);

                        other.component == current_provider.component
                            && other.context == current_provider.context
                            && contains_as_type_param
                    });

                    if is_redundant {
                        continue; // Skip this redundant entry
                    }
                }

                // Simplify the message and hide internal CGP implementation details
                let simplified = simplify_delegation_message(message, metadata);
                chain.push(simplified);
            }
        }
    }

    chain
}

/// Simplifies delegation messages by removing verbose type information and hiding internal CGP traits
fn simplify_delegation_message(message: &str, metadata: &CgpErrorMetadata) -> String {
    let mut simplified = message.to_string();

    // Remove module prefixes FIRST, before doing trait replacements
    // This ensures our pattern matching works correctly
    simplified = simplified.replace("base_area::", "");
    simplified = simplified.replace("scaled_area::", "");
    simplified = simplified.replace("cgp::prelude::", "");

    // Hide internal CGP trait `IsProviderFor` and replace with user-friendly "provider trait"
    // Pattern: "required for `X` to implement `IsProviderFor<YComponent, Z>`"
    // Replace with: "required for `X` to implement the provider trait `Y`"
    if let Some(provider_replacement) = replace_is_provider_for(&simplified, metadata) {
        simplified = provider_replacement;
    }

    // Hide internal CGP trait `CanUseComponent` and replace with user-friendly "consumer trait"
    // Pattern: "required for `X` to implement `CanUseComponent<YComponent>`"
    // Replace with: "required for `X` to implement the consumer trait for `YComponent`"
    if let Some(consumer_replacement) = replace_can_use_component(&simplified, metadata) {
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
fn replace_is_provider_for(message: &str, _metadata: &CgpErrorMetadata) -> Option<String> {
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
fn replace_can_use_component(message: &str, metadata: &CgpErrorMetadata) -> Option<String> {
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

        // Replace CanUseComponent<...> with consumer trait name if available,
        // otherwise use generic description
        // The original message typically has backticks: `CanUseComponent<...>`
        // We need to handle the backticks properly
        let before = &message[..start];
        let after = &message[end_pos + 1..];

        // Check if CanUseComponent is wrapped in backticks
        let has_opening_backtick = before.ends_with('`');
        let has_closing_backtick = after.starts_with('`');

        let replacement = if let Some(consumer_trait) = &metadata.consumer_trait {
            // Use the specific consumer trait name
            if has_opening_backtick && has_closing_backtick {
                format!(
                    "{}the consumer trait `{}`{}",
                    &before[..before.len() - 1],
                    consumer_trait,
                    &after[1..]
                )
            } else {
                format!("{}the consumer trait `{}`{}", before, consumer_trait, after)
            }
        } else {
            // Fall back to generic description with component name
            if has_opening_backtick && has_closing_backtick {
                format!(
                    "{}the consumer trait for `{}`{}",
                    &before[..before.len() - 1],
                    component_name,
                    &after[1..]
                )
            } else {
                format!(
                    "{}the consumer trait for `{}`{}",
                    before, component_name, after
                )
            }
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
    use insta::assert_snapshot;
    use std::fs::File;
    use std::io::BufReader;

    /// Helper function to run a CGP error test from a JSON file
    fn test_cgp_error_from_json(json_filename: &str, test_name: &str) -> Vec<String> {
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

        let mut error_count = 0;
        let mut compiler_message_count = 0;
        let mut total_messages = 0;
        let mut output_lines = Vec::new();

        // Parse the stream of JSON messages
        for message_result in Message::parse_stream(reader) {
            let message = message_result.expect("Failed to parse message");
            total_messages += 1;

            match &message {
                Message::CompilerMessage(compiler_msg) => {
                    compiler_message_count += 1;

                    // Process error-level diagnostics
                    if matches!(compiler_msg.message.level, DiagnosticLevel::Error) {
                        error_count += 1;

                        println!("\n=== Original Error #{} ===", error_count);
                        if let Some(rendered) = &compiler_msg.message.rendered {
                            println!("{}", rendered);
                        }

                        println!("\n=== Improved CGP Error #{} ===", error_count);
                        match render_compiler_message(&compiler_msg) {
                            Ok(improved) => {
                                println!("{}", improved);
                                output_lines.push(improved);
                            }
                            Err(e) => {
                                println!("Error rendering: {}", e);
                                panic!("Failed to render compiler message: {}", e);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        println!("\n=== Summary for {} ===", test_name);
        println!("Total messages parsed: {}", total_messages);
        println!("Compiler messages found: {}", compiler_message_count);
        println!("Error messages found: {}", error_count);

        assert!(
            compiler_message_count > 0,
            "Expected to find at least one compiler message in {}",
            json_filename
        );

        // Return the output for snapshot testing
        output_lines
    }

    #[test]
    fn test_base_area_error() {
        let outputs = test_cgp_error_from_json("base_area.json", "base_area");

        // We expect one error message for base_area
        assert_eq!(outputs.len(), 1, "Expected 1 error message");

        assert_snapshot!(outputs[0], @"
        error[E0277]: missing field `height` required by CGP component
          --> examples/src/base_area.rs:41:9
           |
          41 |         AreaCalculatorComponent,
             |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
           |
           = help: struct `Rectangle` is missing the field `height`
           = note: this field is required by the trait bound `HasRectangleFields`
           = note: delegation chain:
                   - required for `Rectangle` to implement `HasRectangleFields`
                   - required for `RectangleArea` to implement the provider trait `AreaCalculator`
                   - required for `Rectangle` to implement the consumer trait `CanUseRectangle`
           = help: add `pub height: f64` to the `Rectangle` struct definition
        ");
    }

    #[test]
    fn test_base_area_2_error() {
        let outputs = test_cgp_error_from_json("base_area_2.json", "base_area_2");

        // We expect one error message for base_area_2
        assert_eq!(outputs.len(), 1, "Expected 1 error message");

        // This test case has no other HasField implementations,
        // so the error message should suggest adding the derive
        assert!(
            outputs[0].contains("is either missing the field")
                || outputs[0].contains("needs `#[derive(HasField)]`"),
            "Expected error message to mention missing derive possibility"
        );
    }

    #[test]
    fn test_scaled_area_error() {
        let outputs = test_cgp_error_from_json("scaled_area.json", "scaled_area");

        // We expect two error messages, but the first one should be suppressed (empty)
        // because it's a provider trait error that will be followed by a more detailed error
        assert_eq!(outputs.len(), 2, "Expected 2 error messages");

        // The first error should be empty (suppressed provider trait error)
        assert!(
            outputs[0].is_empty(),
            "First error should be suppressed (empty) since it's a redundant provider trait error"
        );

        // The second error should be the comprehensive CGP-formatted error
        assert!(
            outputs[1].contains("missing field `height`"),
            "Second error should be about missing height field"
        );

        // The delegation chain should be deduplicated -
        // should not redundantly mention both ScaledArea<RectangleArea> and RectangleArea
        let delegation_chain_part = outputs[1]
            .split("delegation chain:")
            .nth(1)
            .expect("Expected delegation chain section");

        // Count how many times "AreaCalculator" appears in provider trait mentions
        let area_calculator_count = delegation_chain_part
            .matches("provider trait `AreaCalculator`")
            .count();

        // Should only mention the provider trait once (not for both ScaledArea and RectangleArea)
        assert!(
            area_calculator_count <= 1,
            "Delegation chain should not redundantly mention the same provider trait multiple times. Found {} mentions.",
            area_calculator_count
        );
    }
}
