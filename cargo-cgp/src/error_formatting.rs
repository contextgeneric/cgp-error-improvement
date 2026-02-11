use miette::{
    GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceOffset, SourceSpan,
};

use crate::cgp_diagnostic::CgpDiagnostic;
use crate::cgp_patterns::{
    ProviderRelationship, derive_provider_trait_name, strip_module_prefixes,
};
use crate::diagnostic_db::DiagnosticEntry;
use crate::root_cause::{deduplicate_delegation_notes, deduplicate_provider_relationships};

/// Node in a dependency tree showing trait requirement relationships
#[derive(Debug, Clone)]
struct DependencyNode {
    /// Description of this requirement
    description: String,
    /// Type of trait (check, consumer, provider, getter)
    trait_type: Option<String>,
    /// Whether this requirement is satisfied
    is_satisfied: Option<bool>,
    /// Child dependencies
    children: Vec<DependencyNode>,
}

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
    let message = if entry.has_other_hasfield_impls {
        format!(
            "missing field `{}` in the context `{}`.",
            formatted_field_name, field_info.target_type
        )
    } else {
        format!(
            "missing field `{}` or `#[derive(HasField)]` in the context `{}`.",
            formatted_field_name, field_info.target_type
        )
    };

    // Build help message with clear sections
    let mut help_sections = Vec::new();

    // Get component name for context
    let component_name = entry
        .component_info
        .as_ref()
        .map(|c| strip_module_prefixes(&c.component_type))
        .filter(|name| !name.contains("IsProviderFor<") && !name.contains("CanUseComponent<"));

    // Section 1: High-level context
    if entry.field_info.is_some() {
        if let Some(comp_name) = &component_name {
            help_sections.push(format!(
                "Context `{}` is missing a required field to use `{}`.",
                field_info.target_type, comp_name
            ));
        } else {
            help_sections.push(format!(
                "Context `{}` is missing a required field.",
                field_info.target_type
            ));
        }
    } else if let Some(comp_name) = &component_name {
        help_sections.push(format!(
            "Context `{}` is missing a required field to use `{}`.",
            field_info.target_type, comp_name
        ));
    }

    // Add note about missing field or derive
    if entry.has_other_hasfield_impls {
        help_sections.push(format!(
            "    note: Missing field: `{}`",
            formatted_field_name
        ));
    } else {
        help_sections.push(format!(
            "    note: Missing field: `{}` or struct needs `#[derive(HasField)]`",
            formatted_field_name
        ));
    }

    help_sections.push(String::new()); // Blank line

    // Section 2: Field name warnings (if applicable)
    if field_info.has_unknown_chars {
        help_sections.push(format!(
            "note: some characters in the field name are hidden by the compiler and shown as '\u{FFFD}'"
        ));
        help_sections.push(String::new());
    }

    // Section 3: Struct location (if we have source span)
    if let Some(span) = &entry.primary_span {
        help_sections.push(format!(
            "The struct `{}` is defined at `{}:{}` but does not have the required field `{}`.",
            field_info.target_type, span.file_name, span.line_start, formatted_field_name
        ));
        help_sections.push(String::new());
    }

    // Section 4: Dependency chain as tree
    if !entry.delegation_notes.is_empty() {
        help_sections.push("Dependency chain:".to_string());
        let tree_lines = format_delegation_chain(entry);
        for line in tree_lines {
            help_sections.push(format!("    {}", line));
        }
        help_sections.push(String::new());
    }

    // Section 5: Inner provider note (for higher-order providers)
    let all_inner_providers = detect_inner_providers(&entry.provider_relationships);
    let deduped_relationships = deduplicate_provider_relationships(&entry.provider_relationships);

    if !all_inner_providers.is_empty() {
        let outer_providers: Vec<_> = deduped_relationships
            .iter()
            .filter(|r| {
                !all_inner_providers
                    .iter()
                    .any(|inner| inner == &r.provider_type)
            })
            .collect();

        if !outer_providers.is_empty() {
            help_sections.push(format!(
                "The error in the higher-order provider `{}` might be caused by its inner provider `{}`.",
                outer_providers[0].provider_type, all_inner_providers[0]
            ));
            help_sections.push(String::new());
        }
    }

    // Section 6: Available fields (optional - requires additional extraction)
    // For now, we skip this since we'd need to parse additional diagnostics to find existing fields

    // Section 7: How to fix
    help_sections.push("To fix this error:".to_string());
    if entry.has_other_hasfield_impls {
        if let Some(span) = &entry.primary_span {
            help_sections.push(format!(
                "    • Add a field `{}` to the `{}` struct at {}:{}",
                field_info.field_name, field_info.target_type, span.file_name, span.line_start
            ));
        } else {
            help_sections.push(format!(
                "    • Add a field `{}` to the `{}` struct",
                field_info.field_name, field_info.target_type
            ));
        }
    } else {
        if let Some(span) = &entry.primary_span {
            help_sections.push(format!(
                "    • If the struct has the field `{}`, add `#[derive(HasField)]` to the struct definition at `{}:{}`",
                field_info.field_name, span.file_name, span.line_start
            ));
        } else {
            help_sections.push(format!(
                "    • If the struct has the field `{}`, add `#[derive(HasField)]` to the struct definition",
                field_info.field_name
            ));
        }
        help_sections.push(format!(
            "    • If the field is missing, add a `{}` field to the struct",
            field_info.field_name
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

/// Renders a dependency tree with box-drawing characters
fn render_dependency_tree(
    node: &DependencyNode,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) -> Vec<String> {
    let mut result = Vec::new();

    // Build the line for this node
    if is_root {
        // Root node has no branch character
        let mut line = node.description.clone();

        // Add trait type annotation if present
        if let Some(ref trait_type) = node.trait_type {
            line.push_str(&format!(" ({})", trait_type));
        }

        result.push(line);
    } else {
        let branch = if is_last { "└─" } else { "├─" };
        let mut line = format!("{}{} requires: {}", prefix, branch, node.description);

        // Add trait type annotation if present
        if let Some(ref trait_type) = node.trait_type {
            line.push_str(&format!(" ({})", trait_type));
        }

        // Add satisfaction marker if present
        if let Some(is_satisfied) = node.is_satisfied {
            line.push_str(if is_satisfied { " ✓" } else { " ✗" });
        }

        result.push(line);
    }

    // Render children with updated prefix
    let child_prefix = if is_root {
        prefix.to_string()
    } else if is_last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };

    for (i, child) in node.children.iter().enumerate() {
        let child_is_last = i == node.children.len() - 1;
        result.extend(render_dependency_tree(
            child,
            &child_prefix,
            child_is_last,
            false,
        ));
    }

    result
}

/// Extracts consumer trait name from component name
/// E.g., "AreaCalculatorComponent" -> "CanCalculateArea" (via provider -> consumer mapping)
fn extract_consumer_trait_from_component(component_name: &str) -> Option<String> {
    // Try to derive provider name first
    let _provider = derive_provider_trait_name(component_name)?;

    // We can't reliably derive the consumer trait from just the component name
    // Consumer traits follow "Can{Action}" pattern but we'd need a mapping
    // For now, keep generic description
    None
}

/// Builds a dependency tree from delegation notes and provider relationships
fn build_dependency_tree(entry: &DiagnosticEntry) -> Option<DependencyNode> {
    // Build root node from check trait
    let check_trait = entry.check_trait.as_ref()?;
    let context_type = entry
        .field_info
        .as_ref()
        .map(|f| f.target_type.clone())
        .or_else(|| {
            // Try to extract from delegation notes
            extract_context_from_notes(&entry.delegation_notes)
        })?;

    let mut root = DependencyNode {
        description: format!("{} for {}", check_trait, context_type),
        trait_type: Some("check trait".to_string()),
        is_satisfied: None,
        children: Vec::new(),
    };

    // Add consumer trait level
    if let Some(component_info) = &entry.component_info {
        // Try to extract consumer trait name from notes, otherwise use component-based description
        let consumer_desc =
            extract_consumer_trait_from_notes(&entry.delegation_notes, &context_type)
                .or_else(|| extract_consumer_trait_from_component(&component_info.component_type))
                .unwrap_or_else(|| {
                    // When consumer trait cannot be found, describe it using the component name
                    // This is clearer than trying to use the provider trait name, which would be incorrect
                    let component_name = strip_module_prefixes(&component_info.component_type);
                    format!(
                        "consumer trait of `{}` for {}",
                        component_name, context_type
                    )
                });

        // Apply module prefix stripping to the final description
        let cleaned_desc = strip_module_prefixes(&consumer_desc);

        let mut consumer_node = DependencyNode {
            description: cleaned_desc,
            trait_type: Some("consumer trait".to_string()),
            is_satisfied: None,
            children: Vec::new(),
        };

        // Add provider level(s)
        let provider_nodes = build_provider_nodes(entry, &context_type);
        consumer_node.children = provider_nodes;

        root.children.push(consumer_node);
    }

    Some(root)
}

/// Builds provider nodes from provider relationships
fn build_provider_nodes(entry: &DiagnosticEntry, context_type: &str) -> Vec<DependencyNode> {
    let deduped_relationships = deduplicate_provider_relationships(&entry.provider_relationships);
    let all_inner_providers = detect_inner_providers(&entry.provider_relationships);

    let mut provider_nodes = Vec::new();

    // Find outer provider (if exists)
    let outer_providers: Vec<_> = deduped_relationships
        .iter()
        .filter(|r| {
            !all_inner_providers
                .iter()
                .any(|inner| inner == &r.provider_type)
        })
        .collect();

    if let Some(outer_rel) = outer_providers.first() {
        // This is a higher-order provider
        if let Some(provider_trait) = &entry
            .component_info
            .as_ref()
            .and_then(|c| c.provider_trait.clone())
        {
            let description = format!(
                "{}<{}> for provider {}",
                provider_trait, context_type, outer_rel.provider_type
            );
            let mut outer_node = DependencyNode {
                description: strip_module_prefixes(&description),
                trait_type: Some("provider trait".to_string()),
                is_satisfied: None,
                children: Vec::new(),
            };

            // Add getter requirements and inner provider as children
            let getter_children = build_getter_nodes(entry, context_type);
            outer_node.children.extend(getter_children);

            // Add inner provider node if exists
            if let Some(inner_provider) = all_inner_providers.first() {
                let inner_desc = format!(
                    "{}<{}> for inner provider {}",
                    provider_trait, context_type, inner_provider
                );
                let inner_node = DependencyNode {
                    description: strip_module_prefixes(&inner_desc),
                    trait_type: Some("provider trait".to_string()),
                    is_satisfied: Some(true), // Inner is OK if outer has the error
                    children: Vec::new(),
                };
                outer_node.children.push(inner_node);
            }

            provider_nodes.push(outer_node);
        }
    } else if let Some(rel) = deduped_relationships.first() {
        // Simple provider (no higher-order)
        if let Some(provider_trait) = &entry
            .component_info
            .as_ref()
            .and_then(|c| c.provider_trait.clone())
        {
            let description = format!(
                "{}<{}> for provider {}",
                provider_trait, context_type, rel.provider_type
            );
            let mut provider_node = DependencyNode {
                description: strip_module_prefixes(&description),
                trait_type: Some("provider trait".to_string()),
                is_satisfied: None,
                children: Vec::new(),
            };

            // Add getter requirements as children
            let getter_children = build_getter_nodes(entry, context_type);
            provider_node.children = getter_children;

            provider_nodes.push(provider_node);
        }
    }

    provider_nodes
}

/// Builds getter trait nodes from delegation notes
fn build_getter_nodes(entry: &DiagnosticEntry, context_type: &str) -> Vec<DependencyNode> {
    let mut getter_nodes = Vec::new();

    // Look for "HasXxx" patterns in delegation notes
    for note in &entry.delegation_notes {
        if let Some(getter_trait) = extract_getter_trait_from_note(note) {
            let mut getter_node = DependencyNode {
                description: format!("{} for {}", getter_trait, context_type),
                trait_type: Some("getter trait".to_string()),
                is_satisfied: None,
                children: Vec::new(),
            };

            // If we have field info, add the field requirement as a child
            // We add it to the first getter trait we find, since that's typically the most relevant one
            if getter_nodes.is_empty() {
                if let Some(field_info) = &entry.field_info {
                    let formatted_field = format_field_name(&field_info.field_name);
                    let field_node = DependencyNode {
                        description: format!(
                            "field `{}` on {}",
                            formatted_field, field_info.target_type
                        ),
                        trait_type: None,
                        is_satisfied: Some(false), // This is the missing field
                        children: Vec::new(),
                    };
                    getter_node.children.push(field_node);
                }
            }

            getter_nodes.push(getter_node);
        }
    }

    getter_nodes
}

/// Extracts consumer trait from delegation notes
fn extract_consumer_trait_from_notes(notes: &[String], context_type: &str) -> Option<String> {
    // Look for "Can*" traits in the notes, but exclude CanUseComponent (that's the check trait wrapper)
    for note in notes {
        if let Some(trait_name) = extract_trait_from_note(note) {
            if trait_name.starts_with("Can") && !trait_name.contains("CanUseComponent") {
                return Some(format!("{} for {}", trait_name, context_type));
            }
        }
    }
    None
}

/// Extracts getter trait name from a delegation note
fn extract_getter_trait_from_note(note: &str) -> Option<String> {
    // Look for "to implement `HasXxx`" pattern
    if let Some(trait_name) = extract_trait_from_note(note) {
        // Only return if it looks like a getter trait (Has*)
        if trait_name.starts_with("Has") {
            return Some(trait_name);
        }
    }
    None
}

/// Extracts any trait name from a delegation note
fn extract_trait_from_note(note: &str) -> Option<String> {
    if let Some(start) = note.find("to implement `") {
        let after_start = start + "to implement `".len();
        if let Some(end) = note[after_start..].find('`') {
            let trait_name = &note[after_start..after_start + end];
            let cleaned = strip_module_prefixes(trait_name);
            // Further clean up IsProviderFor patterns
            if cleaned.starts_with("IsProviderFor<") {
                // Extract the component/trait from IsProviderFor<Component, Context>
                if let Some(inner_start) = cleaned.find('<') {
                    let after_bracket = inner_start + 1;
                    if let Some(comma_pos) = cleaned[after_bracket..].find(',') {
                        // Just return the component part
                        return Some(
                            cleaned[after_bracket..after_bracket + comma_pos]
                                .trim()
                                .to_string(),
                        );
                    }
                }
                // If parsing fails, return None to skip this
                return None;
            }
            return Some(cleaned);
        }
    }
    None
}

/// Extracts any trait name from a delegation note
fn extract_context_from_notes(notes: &[String]) -> Option<String> {
    for note in notes {
        // Look for "for `Type` to implement" pattern
        if let Some(start) = note.find("for `") {
            let after_start = start + 5;
            if let Some(end) = note[after_start..].find("` to") {
                let type_name = &note[after_start..after_start + end];
                return Some(strip_module_prefixes(type_name));
            }
        }
    }
    None
}

/// Formats the delegation chain with better structure and CGP-aware terminology
fn format_delegation_chain(entry: &DiagnosticEntry) -> Vec<String> {
    // Try to build a proper dependency tree
    if let Some(tree) = build_dependency_tree(entry) {
        return render_dependency_tree(&tree, "", true, true);
    }

    // Fallback to old format if tree building fails
    format_delegation_chain_legacy(entry)
}

/// Legacy delegation chain formatting (fallback)
fn format_delegation_chain_legacy(entry: &DiagnosticEntry) -> Vec<String> {
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
