/// Module for identifying root causes from transitive failures
/// This implements the approach described in Chapter 9 of the report
use crate::cgp_patterns::ProviderRelationship;
use crate::diagnostic_db::DiagnosticEntry;

/// Analyzes diagnostic entries to determine their causal priority
pub fn rank_by_causal_priority(_entries: &mut [&DiagnosticEntry]) {
    // Sort entries by priority (higher priority first)
    // Priority order:
    // 1. Entries with field_info (missing fields are root causes)
    // 2. Entries with provider_relationships (trait implementation failures)
    // 3. Other entries

    // We don't actually sort since we just want to identify root causes
    // but we could add scoring here if needed
}

/// Returns true if this entry represents a root cause error
pub fn is_root_cause(entry: &DiagnosticEntry) -> bool {
    // A root cause is typically:
    // 1. Has field_info (missing struct field)
    // 2. Has provider_relationships but no further dependencies
    // 3. Is the most specific error in a chain

    if entry.field_info.is_some() {
        return true;
    }

    // If it has provider relationships, it's explaining a dependency chain
    // This is useful context but may or may not be a root cause
    if !entry.provider_relationships.is_empty() {
        return true;
    }

    false
}

/// Determines if an entry should be suppressed because it's a transitive failure
pub fn is_transitive_failure(entry: &DiagnosticEntry, all_entries: &[&DiagnosticEntry]) -> bool {
    // If this entry has field info, it's not transitive
    if entry.field_info.is_some() {
        return false;
    }

    // Check if there's a root cause at the same location that explains this error
    for other in all_entries {
        if entry_locations_match(entry, other) && other.field_info.is_some() {
            // This entry is explained by a root cause at the same location
            return true;
        }
    }

    false
}

/// Checks if two entries are at the same source location
/// With multiple spans, we check if any spans match
fn entry_locations_match(a: &DiagnosticEntry, b: &DiagnosticEntry) -> bool {
    // If either entry has no spans, they can't match
    if a.primary_spans.is_empty() || b.primary_spans.is_empty() {
        return false;
    }

    // Check if any span from a matches any span from b
    for span_a in &a.primary_spans {
        for span_b in &b.primary_spans {
            if span_a.file_name == span_b.file_name
                && span_a.line_start == span_b.line_start
                && span_a.column_start == span_b.column_start
            {
                return true;
            }
        }
    }

    false
}

/// Deduplicates provider relationships by removing nested redundancies
/// For example, if we have both "ScaledArea<RectangleArea>" and "RectangleArea"
/// implementing the same trait for the same component, we only keep the outer one
pub fn deduplicate_provider_relationships(
    relationships: &[ProviderRelationship],
) -> Vec<ProviderRelationship> {
    let mut deduped = Vec::new();

    for rel in relationships {
        // Check if this relationship is contained within another
        let is_redundant = relationships.iter().any(|other| {
            if rel == other {
                return false; // Don't compare with itself
            }

            // Check if they have the same component and context
            if other.component != rel.component || other.context != rel.context {
                return false;
            }

            // Check if the other provider type contains this one as a type parameter
            is_contained_type_parameter(&rel.provider_type, &other.provider_type)
        });

        if !is_redundant {
            deduped.push(rel.clone());
        }
    }

    deduped
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

/// Deduplicates delegation notes by removing redundant entries
pub fn deduplicate_delegation_notes(notes: &[String]) -> Vec<String> {
    // For now, just remove exact duplicates
    // A more sophisticated approach would detect semantic duplicates
    let mut deduped = Vec::new();

    for note in notes {
        if !deduped.contains(note) {
            deduped.push(note.clone());
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_contained_type_parameter() {
        assert!(is_contained_type_parameter(
            "RectangleArea",
            "ScaledArea<RectangleArea>"
        ));
        assert!(is_contained_type_parameter("Foo", "Wrapper<Foo, Bar>"));
        assert!(is_contained_type_parameter("Bar", "Wrapper<Foo, Bar>"));
        assert!(!is_contained_type_parameter("Baz", "Wrapper<Foo, Bar>"));
        assert!(!is_contained_type_parameter(
            "Area",
            "ScaledArea<RectangleArea>"
        ));
    }

    #[test]
    fn test_deduplicate_provider_relationships() {
        let relationships = vec![
            ProviderRelationship {
                provider_type: "RectangleArea".to_string(),
                component: "AreaCalculatorComponent".to_string(),
                context: "Rectangle".to_string(),
            },
            ProviderRelationship {
                provider_type: "ScaledArea<RectangleArea>".to_string(),
                component: "AreaCalculatorComponent".to_string(),
                context: "Rectangle".to_string(),
            },
        ];

        let deduped = deduplicate_provider_relationships(&relationships);

        // Should only keep the outer one (ScaledArea<RectangleArea>)
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].provider_type, "ScaledArea<RectangleArea>");
    }
}
