use miette::{
    GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceOffset, SourceSpan,
};

use crate::cgp_diagnostic::CgpDiagnostic;
use crate::cgp_patterns::{
    ComponentInfo, ProviderRelationship, derive_provider_trait_name, strip_module_prefixes,
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
    /// Whether this node is a reference to an earlier node (shown with (*) marker)
    /// Used in flattened dependency trees to avoid duplicating subtrees
    is_reference: bool,
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

    // Get component names for context
    // If we have multiple components, we'll list them all
    let component_names: Vec<String> = entry
        .component_infos
        .iter()
        .map(|c| strip_module_prefixes(&c.component_type))
        .filter(|name| !name.contains("IsProviderFor<") && !name.contains("CanUseComponent<"))
        .collect();

    // Section 1: High-level context
    if entry.field_info.is_some() {
        if !component_names.is_empty() {
            if component_names.len() == 1 {
                help_sections.push(format!(
                    "Context `{}` is missing a required field to use `{}`.",
                    field_info.target_type, component_names[0]
                ));
            } else {
                // Multiple components affected
                let components_list = component_names.join("`, `");
                help_sections.push(format!(
                    "Context `{}` is missing a required field to use multiple components: `{}`.",
                    field_info.target_type, components_list
                ));
            }
        } else {
            help_sections.push(format!(
                "Context `{}` is missing a required field.",
                field_info.target_type
            ));
        }
    } else if !component_names.is_empty() {
        if component_names.len() == 1 {
            help_sections.push(format!(
                "Context `{}` is missing a required field to use `{}`.",
                field_info.target_type, component_names[0]
            ));
        } else {
            let components_list = component_names.join("`, `");
            help_sections.push(format!(
                "Context `{}` is missing a required field to use multiple components: `{}`.",
                field_info.target_type, components_list
            ));
        }
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
    // Use the first span if available
    if let Some(span) = entry.primary_spans.first() {
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
        if let Some(span) = entry.primary_spans.first() {
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
        if let Some(span) = entry.primary_spans.first() {
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
        help_sections.push(String::new()); // Blank line
    }

    // Check for nested consumer traits and add help message for indirect components
    let nested_consumers = extract_nested_consumer_traits(&entry.delegation_notes);
    if !nested_consumers.is_empty() {
        // Get the context type from the unsatisfied provider or delegation notes
        let context_type = extract_unsatisfied_provider_from_message(&entry.message)
            .map(|u| u.context_type)
            .or_else(|| extract_context_from_notes(&entry.delegation_notes))
            .unwrap_or_else(|| "the context".to_string());

        // For each nested consumer trait, suggest checking its component
        for nested_consumer in &nested_consumers {
            if let Some(component_name) =
                derive_component_from_consumer_trait(&nested_consumer.trait_name)
            {
                help_sections.push(format!(
                    "Add a check that `{}` can use `{}` using `check_components!` to get further details on the missing dependencies.",
                    context_type,
                    component_name
                ));
            }
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
/// When there are multiple components, creates a label for each span
fn build_source_and_labels(
    entry: &DiagnosticEntry,
) -> (Option<NamedSource<String>>, Vec<LabeledSpan>) {
    if entry.primary_spans.is_empty() {
        return (None, vec![]);
    }

    // Use the first span to determine the file
    let first_span = &entry.primary_spans[0];

    // Try to read the actual source file to get proper content and offsets
    // The file_name might be absolute or relative
    let file_result = std::fs::read_to_string(&first_span.file_name).or_else(|_| {
        // If the path is relative, try from the workspace root
        // Look for common workspace patterns
        if let Ok(current_dir) = std::env::current_dir() {
            // Try current directory first
            let candidate1 = current_dir.join(&first_span.file_name);
            if let Ok(content) = std::fs::read_to_string(&candidate1) {
                return Ok(content);
            }

            // Try parent directory (in case we're in a subdirectory)
            if let Some(parent) = current_dir.parent() {
                let candidate2 = parent.join(&first_span.file_name);
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
            let source_code = NamedSource::new(&first_span.file_name, file_content.clone());

            // Create a labeled span for each primary span
            let mut labels = Vec::new();

            for span in &entry.primary_spans {
                // Calculate byte offset in the actual file
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
                    .unwrap_or_else(|| "unsatisfied trait bound".to_string());

                let labeled_span = LabeledSpan::new_with_span(
                    Some(label_text),
                    SourceSpan::new(SourceOffset::from(byte_offset), span_length),
                );

                labels.push(labeled_span);
            }

            (Some(source_code), labels)
        }
        Err(_) => {
            // Fallback: reconstruct from span text of the first span
            let source_text = first_span
                .text
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            if source_text.is_empty() {
                // If we have no source text at all, just return nothing
                return (None, vec![]);
            }

            let source_code = NamedSource::new(&first_span.file_name, source_text);

            // For fallback, create simple labels for each span
            let mut labels = Vec::new();

            for span in &entry.primary_spans {
                let byte_offset = span.column_start.saturating_sub(1);
                let span_length = span.column_end.saturating_sub(span.column_start).max(1);

                let label_text = span
                    .label
                    .clone()
                    .unwrap_or_else(|| "unsatisfied trait bound".to_string());

                let labeled_span = LabeledSpan::new_with_span(
                    Some(label_text),
                    SourceSpan::new(SourceOffset::from(byte_offset), span_length),
                );

                labels.push(labeled_span);
            }

            (Some(source_code), labels)
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
        let mut line = format!("{}{} {}", prefix, branch, node.description);

        // Add trait type annotation if present
        if let Some(ref trait_type) = node.trait_type {
            line.push_str(&format!(" ({})", trait_type));
        }

        // Add satisfaction marker if present
        if let Some(is_satisfied) = node.is_satisfied {
            line.push_str(if is_satisfied { " ✓" } else { " ✗" });
        }

        // If this is a reference node, add (*) marker
        // This indicates the full tree is shown elsewhere (cargo tree style)
        if node.is_reference {
            line.push_str(" (*)");
        }

        result.push(line);
    }

    // If this is a reference node, don't render children
    // The full tree is shown at the root level
    if node.is_reference {
        return result;
    }

    // Render children with updated prefix
    let child_prefix = if is_root {
        prefix.to_string()
    } else if is_last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };

    // Render all children normally
    // All children are treated the same in the flattened structure
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

/// Derives a component name from a consumer trait name
/// E.g., "CanCalculateArea" -> "AreaCalculatorComponent"
/// This is a heuristic that works for common CGP naming patterns:
/// - Consumer trait: Can{Action} (e.g., CanCalculateArea)
/// - Component: {Action}Component (e.g., AreaCalculatorComponent)
fn derive_component_from_consumer_trait(consumer_trait: &str) -> Option<String> {
    // Check if it starts with "Can"
    if let Some(action_part) = consumer_trait.strip_prefix("Can") {
        // Remove the "Can" prefix and append "Component"
        // E.g., "CalculateArea" -> "AreaCalculatorComponent"
        Some(format!("{}Component", action_part))
    } else {
        None
    }
}

/// Finds the actual consumer trait name for a given component
/// by looking it up in the diagnostic entry's consumer trait dependencies
///
/// This avoids the need to derive consumer trait names from component names,
/// which is error-prone due to naming variations. Instead, we use the actual
/// trait names extracted from the compiler diagnostics.
///
/// Returns None if no matching consumer trait is found for this component.
fn find_consumer_trait_for_component(
    component_name: &str,
    entry: &DiagnosticEntry,
) -> Option<String> {
    // Check each consumer trait dependency to see if it matches this component
    for dep in &entry.consumer_trait_dependencies {
        if let Some(ref derived_component) = dep.component_name {
            // Match by the component name derived from the consumer trait
            if derived_component == component_name {
                return Some(dep.trait_name.clone());
            }
        }
    }

    // Also check provider relationships to find the provider trait,
    // then search for consumer traits that might correspond to it
    for provider_rel in &entry.provider_relationships {
        if strip_module_prefixes(&provider_rel.component) == component_name {
            // Found the provider for this component
            // Now look for consumer traits that might match this provider's trait
            // This is a fallback heuristic when the component_name derivation doesn't match

            // The heuristic here is fuzzy matching based on significant words
            // For example: AreaCalculator (provider) ~ CanCalculateArea (consumer)
            // We look for shared words between them

            let provider_trait = derive_provider_trait_name(component_name)?;
            let provider_words: Vec<&str> = provider_trait
                .split(|c: char| c.is_uppercase())
                .filter(|s| !s.is_empty() && s.len() > 2)
                .collect();

            for dep in &entry.consumer_trait_dependencies {
                let consumer_words: Vec<&str> = dep
                    .trait_name
                    .strip_prefix("Can")
                    .unwrap_or(&dep.trait_name)
                    .split(|c: char| c.is_uppercase())
                    .filter(|s| !s.is_empty() && s.len() > 2)
                    .collect();

                // Check if they share significant words
                for provider_word in &provider_words {
                    for consumer_word in &consumer_words {
                        if provider_word.eq_ignore_ascii_case(consumer_word) {
                            return Some(dep.trait_name.clone());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Matches a component to its specific provider relationship from the error
/// This is based on the IsProviderFor<Component, Context> notes in the diagnostics
fn match_component_to_provider<'a>(
    component_info: &ComponentInfo,
    provider_relationships: &'a [ProviderRelationship],
) -> Option<&'a ProviderRelationship> {
    let component_name = strip_module_prefixes(&component_info.component_type);

    // Try exact match first
    for rel in provider_relationships {
        if strip_module_prefixes(&rel.component) == component_name {
            return Some(rel);
        }
    }

    // If no exact match, try matching by provider trait name
    // The provider trait should be derivable from the component name
    if let Some(ref provider_trait) = component_info.provider_trait {
        for rel in provider_relationships {
            // Check if this relationship's component derives the same provider trait
            if let Some(rel_provider_trait) = derive_provider_trait_name(&rel.component) {
                if rel_provider_trait == *provider_trait {
                    return Some(rel);
                }
            }
        }
    }

    None
}

/// Builds a dependency tree from delegation notes and provider relationships
/// When there are multiple components, shows them as siblings at the root level (flattened structure)
/// This creates a cargo-tree-style view where shared dependencies are marked with (*)
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
        // Wrap trait and type names in backticks for consistent code construct formatting
        // Rationale: Backticks visually distinguish code elements from descriptive text
        description: format!("`{}` for `{}`", check_trait, context_type),
        trait_type: Some("check trait".to_string()),
        is_satisfied: None,
        is_reference: false,
        children: Vec::new(),
    };

    // Track which consumer traits have been rendered to avoid duplicating full trees
    // This implements the flattened dependency view similar to cargo tree
    let mut rendered_consumer_traits: Vec<String> = Vec::new();

    // Process all components in order, showing them as siblings at the root level
    // This is the key change for flattened dependency trees
    for component_info in &entry.component_infos {
        let component_name = strip_module_prefixes(&component_info.component_type);

        // Try to find the actual consumer trait name for this component
        // If found, use it directly; otherwise fall back to generic description
        let (consumer_desc, consumer_trait_name) =
            if let Some(trait_name) = find_consumer_trait_for_component(&component_name, entry) {
                // Found the actual consumer trait - use it directly
                // Wrap both trait name and context type in backticks
                let desc = format!("`{}` for `{}`", trait_name, context_type);
                (desc, Some(trait_name.clone()))
            } else {
                // Fallback to generic description
                // Note: component_name and context_type are already wrapped in backticks
                let desc = format!(
                    "consumer trait of `{}` for `{}`",
                    component_name, context_type
                );
                (desc, None)
            };

        let mut consumer_node = DependencyNode {
            description: consumer_desc,
            trait_type: Some("consumer trait".to_string()),
            is_satisfied: None,
            is_reference: false,
            children: Vec::new(),
        };

        // Match this component to its specific provider relationship
        if let Some(provider_rel) =
            match_component_to_provider(component_info, &entry.provider_relationships)
        {
            // Build provider node for this specific relationship
            // Pass the rendered_consumer_traits and current consumer trait name
            // to avoid showing the component's own consumer trait as a nested dependency
            let provider_nodes = build_provider_nodes_for_component(
                entry,
                &context_type,
                Some(component_info),
                Some(provider_rel),
                &rendered_consumer_traits,
                consumer_trait_name.as_deref(),
            );
            consumer_node.children = provider_nodes;
        } else {
            // Fallback: build without specific provider relationship
            let provider_nodes = build_provider_nodes_for_component(
                entry,
                &context_type,
                Some(component_info),
                None,
                &rendered_consumer_traits,
                consumer_trait_name.as_deref(),
            );
            consumer_node.children = provider_nodes;
        }

        // Track this consumer trait as rendered (if we know its name)
        if let Some(trait_name) = consumer_trait_name {
            rendered_consumer_traits.push(trait_name);
        }

        root.children.push(consumer_node);
    }

    // If no component info, try building without it (fallback)
    if entry.component_infos.is_empty() && !entry.provider_relationships.is_empty() {
        let provider_nodes =
            build_provider_nodes_for_component(entry, &context_type, None, None, &Vec::new(), None);
        root.children.extend(provider_nodes);
    }

    Some(root)
}

/// Builds provider nodes for a specific component and its provider relationship
/// If component_info is None, builds nodes based on provider relationships alone
/// If provider_rel is provided, uses that specific relationship; otherwise uses first available
/// The rendered_consumer_traits parameter tracks which consumer traits have already been shown
/// at the root level, so we can mark them as references (*) instead of duplicating the full tree
/// The current_consumer_trait parameter is the consumer trait of the current component,
/// which should be excluded from nested consumer traits to avoid showing it as its own dependency
fn build_provider_nodes_for_component(
    entry: &DiagnosticEntry,
    context_type: &str,
    component_info: Option<&ComponentInfo>,
    provider_rel: Option<&ProviderRelationship>,
    rendered_consumer_traits: &[String],
    current_consumer_trait: Option<&str>,
) -> Vec<DependencyNode> {
    let mut provider_nodes = Vec::new();

    // Determine which provider relationship to use
    let all_inner_providers = detect_inner_providers(&entry.provider_relationships);
    let deduped_relationships = deduplicate_provider_relationships(&entry.provider_relationships);

    let rel_to_use = if let Some(rel) = provider_rel {
        Some(rel)
    } else {
        deduped_relationships.first()
    };

    if let Some(rel) = rel_to_use {
        if let Some(provider_trait) = component_info.and_then(|c| c.provider_trait.clone()) {
            // Check if this is a higher-order provider (has inner providers)
            let is_higher_order = all_inner_providers
                .iter()
                .any(|inner| is_contained_type_parameter(inner, &rel.provider_type));

            // Wrap all code constructs in backticks: provider trait, context type, and provider type
            let description = format!(
                "`{}<{}>` for provider `{}`",
                provider_trait, context_type, rel.provider_type
            );
            let mut provider_node = DependencyNode {
                description: strip_module_prefixes(&description),
                trait_type: Some("provider trait".to_string()),
                is_satisfied: None,
                is_reference: false,
                children: Vec::new(),
            };

            // Add nested consumer trait dependencies (transitive dependencies)
            // These are consumer traits that this provider depends on
            // Filter out the current component's own consumer trait to avoid showing it as its own dependency
            let all_nested_consumers: Vec<_> =
                extract_nested_consumer_traits(&entry.delegation_notes)
                    .into_iter()
                    .filter(|nested| {
                        // Exclude the current component's consumer trait
                        if let Some(current_trait) = current_consumer_trait {
                            nested.trait_name != current_trait
                        } else {
                            true
                        }
                    })
                    .collect();
            let has_nested_consumer_deps = !all_nested_consumers.is_empty();

            // Add getter requirements as children (if this provider directly requires fields)
            // Only add getters if there's no nested consumer trait (to avoid duplication)
            if !has_nested_consumer_deps {
                let getter_children = build_getter_nodes(entry, context_type);
                provider_node.children.extend(getter_children);
            }

            // Add all nested consumer dependencies
            for nested_consumer in &all_nested_consumers {
                // Build nodes for the nested consumer + its provider tree
                // Pass rendered_consumer_traits to mark references as needed
                let nested_nodes = build_nested_consumer_provider_nodes(
                    entry,
                    nested_consumer,
                    context_type,
                    rendered_consumer_traits,
                );
                provider_node.children.extend(nested_nodes);
            }

            // If this is a higher-order provider, add inner provider as info node
            if is_higher_order {
                if let Some(inner_provider) = all_inner_providers.first() {
                    // Wrap inner provider description with backticks
                    let inner_desc = format!(
                        "`{}<{}>` for inner provider `{}`",
                        provider_trait, context_type, inner_provider
                    );
                    let inner_node = DependencyNode {
                        description: strip_module_prefixes(&inner_desc),
                        trait_type: Some("provider trait".to_string()),
                        is_satisfied: Some(true), // Inner is OK if outer has the error
                        is_reference: false,
                        children: Vec::new(),
                    };
                    provider_node.children.push(inner_node);
                }
            }

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
                // Wrap getter trait name and context type in backticks
                description: format!("`{}` for `{}`", getter_trait, context_type),
                trait_type: Some("getter trait".to_string()),
                is_satisfied: None,
                is_reference: false,
                children: Vec::new(),
            };

            // If we have field info, add the field requirement as a child
            // We add it to the first getter trait we find, since that's typically the most relevant one
            if getter_nodes.is_empty() {
                if let Some(field_info) = &entry.field_info {
                    let formatted_field = format_field_name(&field_info.field_name);
                    let field_node = DependencyNode {
                        // Wrap both field name and target type in backticks
                        description: format!(
                            "field `{}` on `{}`",
                            formatted_field, field_info.target_type
                        ),
                        trait_type: None,
                        is_satisfied: Some(false), // This is the missing field
                        is_reference: false,
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

/// Builds nodes for nested consumer+provider dependencies
/// This handles cases where a provider depends on another consumer trait,
/// which in turn requires another provider that's not satisfied.
///
/// For example: DensityFromMassField -> requires CanCalculateArea -> requires RectangleArea
///
/// If the nested consumer is in rendered_consumer_traits, marks it as a reference (*)
/// instead of building the full tree to avoid duplication (cargo tree style).
fn build_nested_consumer_provider_nodes(
    entry: &DiagnosticEntry,
    nested_consumer: &NestedConsumerTrait,
    _parent_context_type: &str,
    rendered_consumer_traits: &[String],
) -> Vec<DependencyNode> {
    let mut nodes = Vec::new();

    // Check if this consumer trait has already been rendered at the root level
    // If so, mark it as a reference instead of building the full tree
    let is_reference = rendered_consumer_traits
        .iter()
        .any(|rendered| *rendered == nested_consumer.trait_name);

    // Check if this consumer trait maps to a checked component (appears in component_infos)
    // We match by checking if the provider trait of any component matches this consumer trait
    // For example: CanCalculateArea consumer trait → AreaCalculator provider trait
    let matching_component = entry.component_infos.iter().find(|comp| {
        if let Some(ref provider_trait) = comp.provider_trait {
            // Extract the action/noun part from consumer trait (e.g., "CanCalculateArea" → "CalculateArea")
            // and check if it matches the provider trait "AreaCalculator"
            // This is a fuzzy match since the names follow similar patterns but aren't exact
            if let Some(action_part) = nested_consumer.trait_name.strip_prefix("Can") {
                // Both should contain similar words, just potentially in different order
                // Simple heuristic: check if provider trait contains any significant word from action part
                let action_words: Vec<&str> = action_part
                    .split(|c: char| c.is_uppercase())
                    .filter(|s| !s.is_empty() && s.len() > 2)
                    .collect();

                let provider_words: Vec<&str> = provider_trait
                    .split(|c: char| c.is_uppercase())
                    .filter(|s| !s.is_empty() && s.len() > 2)
                    .collect();

                // Check if they share significant words
                for action_word in &action_words {
                    for provider_word in &provider_words {
                        if action_word.eq_ignore_ascii_case(provider_word) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    });

    let is_shared_component = matching_component.is_some();

    // Create a node for the nested consumer trait
    // Wrap consumer trait name and context type in backticks
    let consumer_desc = format!(
        "`{}` for `{}`",
        nested_consumer.trait_name, nested_consumer.context_type
    );
    let mut consumer_node = DependencyNode {
        description: consumer_desc,
        trait_type: Some("consumer trait".to_string()),
        is_satisfied: None,
        children: Vec::new(),
        is_reference, // Mark if it's a reference to an earlier node
    };

    // If this is a reference, don't build children - the full tree is shown elsewhere
    if is_reference {
        nodes.push(consumer_node);
        return nodes;
    }

    if is_shared_component {
        // This is a shared component dependency that's also checked at root level
        // Build the full provider tree for it, including getter and field requirements

        if let Some(component_info) = matching_component {
            // Match this component to its provider relationship
            if let Some(provider_rel) =
                match_component_to_provider(component_info, &entry.provider_relationships)
            {
                if let Some(provider_trait) = component_info.provider_trait.clone() {
                    // Wrap all code constructs in backticks
                    let provider_desc = format!(
                        "`{}<{}>` for provider `{}`",
                        provider_trait, nested_consumer.context_type, provider_rel.provider_type
                    );

                    let mut provider_node = DependencyNode {
                        description: strip_module_prefixes(&provider_desc),
                        trait_type: Some("provider trait".to_string()),
                        is_satisfied: None,
                        children: Vec::new(),
                        is_reference: false,
                    };

                    // Add getter requirements and field nodes for this provider
                    // This ensures the full dependency tree is shown, including the missing field
                    let getter_children = build_getter_nodes(entry, &nested_consumer.context_type);
                    provider_node.children.extend(getter_children);

                    consumer_node.children.push(provider_node);
                }
            }
        }
    } else {
        // Not a shared component - this consumer trait is not checked at root
        // Just show that it's not satisfied, don't build a full tree
        // Try to extract the unsatisfied provider from the main error message
        if let Some(unsatisfied) = extract_unsatisfied_provider_from_message(&entry.message) {
            // Create a provider node that's marked as unsatisfied
            // Wrap all code constructs in backticks
            let provider_desc = format!(
                "`{}<{}>` for provider `{}`",
                unsatisfied.trait_name, unsatisfied.context_type, unsatisfied.provider_type
            );

            let provider_node = DependencyNode {
                description: strip_module_prefixes(&provider_desc),
                trait_type: Some("provider trait".to_string()),
                is_satisfied: Some(false), // Mark as unsatisfied
                children: Vec::new(),
                is_reference: false,
            };

            consumer_node.children.push(provider_node);
        }
    }

    nodes.push(consumer_node);
    nodes
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

/// Information about a nested consumer trait dependency extracted from delegation notes
/// This represents a consumer trait that a provider depends on
#[derive(Debug, Clone)]
struct NestedConsumerTrait {
    /// The consumer trait name (e.g., "CanCalculateArea")
    trait_name: String,
    /// The context type (e.g., "Rectangle")
    context_type: String,
}

/// Extracts nested consumer trait dependencies from delegation notes
/// These are consumer traits that providers depend on, shown in notes like:
/// "required for `Rectangle` to implement `CanCalculateArea`"
fn extract_nested_consumer_traits(notes: &[String]) -> Vec<NestedConsumerTrait> {
    let mut results = Vec::new();

    for note in notes {
        // Look for pattern: "required for `Context` to implement `TraitName`"
        // This indicates that the provider depends on this consumer trait
        if let Some(for_pos) = note.find("required for `") {
            let after_for = for_pos + "required for `".len();

            // Extract context type (between first ` and next `)
            if let Some(context_end) = note[after_for..].find('`') {
                let context_type = &note[after_for..after_for + context_end];

                // Look for the trait name after "to implement `"
                if let Some(implement_pos) = note[after_for + context_end..].find("to implement `")
                {
                    let trait_start =
                        after_for + context_end + implement_pos + "to implement `".len();

                    if let Some(trait_end) = note[trait_start..].find('`') {
                        let trait_name = &note[trait_start..trait_start + trait_end];

                        // Filter out internal CGP traits - we only want consumer traits
                        // Consumer traits typically start with "Can" but exclude framework traits
                        let cleaned_trait = strip_module_prefixes(trait_name);

                        // Skip if it's an IsProviderFor or CanUseComponent trait (these are internal)
                        if cleaned_trait.starts_with("Can")
                            && !cleaned_trait.contains("CanUseComponent")
                            && !cleaned_trait.starts_with("IsProviderFor")
                        {
                            results.push(NestedConsumerTrait {
                                trait_name: cleaned_trait,
                                context_type: strip_module_prefixes(context_type),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

/// Information about an unsatisfied provider trait extracted from the error message
#[derive(Debug, Clone)]
struct UnsatisfiedProvider {
    /// The provider type (e.g., "RectangleArea")
    provider_type: String,
    /// The trait that's not satisfied (e.g., "AreaCalculator")
    trait_name: String,
    /// The context type (e.g., "Rectangle")
    context_type: String,
}

/// Extracts unsatisfied provider information from the main error message
/// Error messages follow the pattern:
/// "the trait bound `ProviderType: TraitName<Context>` is not satisfied"
fn extract_unsatisfied_provider_from_message(message: &str) -> Option<UnsatisfiedProvider> {
    // Look for pattern: "the trait bound `Provider: Trait<Context>` is not satisfied"
    if let Some(bound_start) = message.find("the trait bound `") {
        let after_bound = bound_start + "the trait bound `".len();

        // Find the closing backtick
        if let Some(bound_end) = message[after_bound..].find("` is not satisfied") {
            let bound_str = &message[after_bound..after_bound + bound_end];

            // Parse "Provider: Trait<Context>"
            if let Some(colon_pos) = bound_str.find(": ") {
                let provider_type = bound_str[..colon_pos].trim();
                let trait_and_context = bound_str[colon_pos + 2..].trim();

                // Parse "Trait<Context>"
                if let Some(open_bracket) = trait_and_context.find('<') {
                    let trait_name = trait_and_context[..open_bracket].trim();

                    // Extract context (everything between < and >)
                    if let Some(close_bracket) = trait_and_context.find('>') {
                        let context_type =
                            trait_and_context[open_bracket + 1..close_bracket].trim();

                        return Some(UnsatisfiedProvider {
                            provider_type: strip_module_prefixes(provider_type),
                            trait_name: strip_module_prefixes(trait_name),
                            context_type: strip_module_prefixes(context_type),
                        });
                    }
                }
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
