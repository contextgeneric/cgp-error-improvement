/// Module for building an internal database of diagnostics and merging related errors
/// This implements the approach described in Chapters 7-8 of the report
use cargo_metadata::diagnostic::{Diagnostic, DiagnosticLevel, DiagnosticSpan};
use std::collections::HashMap;

use crate::cgp_patterns::{
    ComponentInfo, FieldInfo, ProviderRelationship, extract_component_info, extract_consumer_trait,
    extract_field_info, extract_provider_relationship, has_other_hasfield_implementations,
};

/// A database that collects and merges related diagnostic information
#[derive(Debug, Default)]
pub struct DiagnosticDatabase {
    /// Map from diagnostic key to merged diagnostic entry
    entries: HashMap<DiagnosticKey, DiagnosticEntry>,
}

/// Key used to identify and group related diagnostics
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DiagnosticKey {
    /// Primary source location (file:line:column)
    location: SourceLocation,
    /// Component involved (if any)
    component: Option<String>,
}

/// Source code location
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceLocation {
    file: String,
    line: usize,
    column: usize,
}

impl SourceLocation {
    fn from_span(span: &DiagnosticSpan) -> Self {
        SourceLocation {
            file: span.file_name.clone(),
            line: span.line_start,
            column: span.column_start,
        }
    }
}

/// A merged diagnostic entry combining information from multiple related errors
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    /// The original diagnostic (we keep the first one as the primary)
    pub original: Diagnostic,

    /// Extracted field information (missing field errors)
    pub field_info: Option<FieldInfo>,

    /// Component information
    pub component_info: Option<ComponentInfo>,

    /// Consumer trait name (from "required by a bound in")
    pub consumer_trait: Option<String>,

    /// Provider relationships extracted from error chain
    pub provider_relationships: Vec<ProviderRelationship>,

    /// Delegation chain notes (raw, for later processing)
    pub delegation_notes: Vec<String>,

    /// Whether this type has other HasField implementations
    pub has_other_hasfield_impls: bool,

    /// Primary span for error reporting
    pub primary_span: Option<DiagnosticSpan>,

    /// Error code (e.g., "E0277")
    pub error_code: Option<String>,

    /// Main error message
    pub message: String,

    /// Whether this is a root cause or a transitive error
    pub is_root_cause: bool,

    /// Whether this error should be suppressed (because it's redundant)
    pub suppressed: bool,
}

impl DiagnosticDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// First pass: Add a diagnostic to the database
    /// If a related diagnostic already exists, merge information
    pub fn add_diagnostic(&mut self, diagnostic: &Diagnostic) {
        // Extract key components for grouping
        let primary_span = diagnostic.spans.iter().find(|s| s.is_primary);

        if primary_span.is_none() {
            // Can't process without a location
            return;
        }

        let location = SourceLocation::from_span(primary_span.unwrap());

        // Extract component info for grouping
        let component_info = extract_component_info(&diagnostic.message).or_else(|| {
            // Try children
            for child in &diagnostic.children {
                if let Some(info) = extract_component_info(&child.message) {
                    return Some(info);
                }
            }
            None
        });

        let key = DiagnosticKey {
            location: location.clone(),
            component: component_info.as_ref().map(|c| c.component_type.clone()),
        };

        // Check if we already have an entry for this key
        if self.entries.contains_key(&key) {
            // Merge new information into existing entry
            Self::merge_diagnostic_info(&mut self.entries, &key, diagnostic);
        } else {
            // Create new entry
            let entry = Self::create_entry(diagnostic, primary_span.unwrap().clone());
            self.entries.insert(key, entry);
        }
    }

    /// Creates a new diagnostic entry from a diagnostic
    fn create_entry(diagnostic: &Diagnostic, primary_span: DiagnosticSpan) -> DiagnosticEntry {
        // Extract all available information
        let field_info = extract_field_info(diagnostic);
        let component_info = Self::extract_component_info_from_diagnostic(diagnostic);
        let consumer_trait = Self::extract_consumer_trait_from_diagnostic(diagnostic);
        let provider_relationships =
            Self::extract_provider_relationships_from_diagnostic(diagnostic);
        let delegation_notes = Self::extract_delegation_notes(diagnostic);
        let has_other_hasfield_impls = has_other_hasfield_implementations(diagnostic);
        let error_code = diagnostic.code.as_ref().map(|c| c.code.clone());

        // Determine if this is a root cause
        // A root cause has field_info (missing field) or is the most specific error
        let is_root_cause = field_info.is_some();

        DiagnosticEntry {
            original: diagnostic.clone(),
            field_info,
            component_info,
            consumer_trait,
            provider_relationships,
            delegation_notes,
            has_other_hasfield_impls,
            primary_span: Some(primary_span),
            error_code,
            message: diagnostic.message.clone(),
            is_root_cause,
            suppressed: false,
        }
    }

    /// Merges information from a new diagnostic into an existing entry
    fn merge_diagnostic_info(
        entries: &mut HashMap<DiagnosticKey, DiagnosticEntry>,
        key: &DiagnosticKey,
        new: &Diagnostic,
    ) {
        if let Some(existing) = entries.get_mut(key) {
            // If the new diagnostic has field info and existing doesn't, add it
            if existing.field_info.is_none() {
                if let Some(field_info) = extract_field_info(new) {
                    existing.field_info = Some(field_info);
                    existing.is_root_cause = true;
                }
            }

            // Merge component info
            if existing.component_info.is_none() {
                existing.component_info = Self::extract_component_info_from_diagnostic(new);
            }

            // Merge consumer trait
            if existing.consumer_trait.is_none() {
                existing.consumer_trait = Self::extract_consumer_trait_from_diagnostic(new);
            }

            // Add new provider relationships
            let new_relationships = Self::extract_provider_relationships_from_diagnostic(new);
            for rel in new_relationships {
                if !existing.provider_relationships.contains(&rel) {
                    existing.provider_relationships.push(rel);
                }
            }

            // Merge delegation notes
            let new_notes = Self::extract_delegation_notes(new);
            for note in new_notes {
                if !existing.delegation_notes.contains(&note) {
                    existing.delegation_notes.push(note);
                }
            }

            // Update hasfield implementations flag
            if !existing.has_other_hasfield_impls {
                existing.has_other_hasfield_impls = has_other_hasfield_implementations(new);
            }

            // If the new diagnostic has an error code and existing doesn't, use it
            if existing.error_code.is_none() {
                existing.error_code = new.code.as_ref().map(|c| c.code.clone());
            }
        }
    }

    /// Extract component info from anywhere in the diagnostic
    fn extract_component_info_from_diagnostic(diagnostic: &Diagnostic) -> Option<ComponentInfo> {
        // Try main message
        if let Some(info) = extract_component_info(&diagnostic.message) {
            return Some(info);
        }

        // Try all children
        for child in &diagnostic.children {
            if let Some(info) = extract_component_info(&child.message) {
                return Some(info);
            }
        }

        None
    }

    /// Extract consumer trait from diagnostic notes
    fn extract_consumer_trait_from_diagnostic(diagnostic: &Diagnostic) -> Option<String> {
        for child in &diagnostic.children {
            if matches!(child.level, DiagnosticLevel::Note) {
                if let Some(trait_name) = extract_consumer_trait(&child.message) {
                    return Some(trait_name);
                }
            }
        }
        None
    }

    /// Extract provider relationships from diagnostic notes
    fn extract_provider_relationships_from_diagnostic(
        diagnostic: &Diagnostic,
    ) -> Vec<ProviderRelationship> {
        let mut relationships = Vec::new();

        for child in &diagnostic.children {
            if matches!(child.level, DiagnosticLevel::Note) {
                if let Some(rel) = extract_provider_relationship(&child.message) {
                    relationships.push(rel);
                }
            }
        }

        relationships
    }

    /// Extract delegation chain notes
    fn extract_delegation_notes(diagnostic: &Diagnostic) -> Vec<String> {
        let mut notes = Vec::new();

        for child in &diagnostic.children {
            if matches!(child.level, DiagnosticLevel::Note) {
                if child.message.contains("required for") && child.message.contains("to implement")
                {
                    notes.push(child.message.clone());
                }
            }
        }

        notes
    }

    /// Second pass: Apply deduplication and suppression logic
    pub fn deduplicate(&mut self) {
        // Find entries that should be suppressed
        let mut keys_to_suppress = Vec::new();

        for (key, entry) in &self.entries {
            // An error should be suppressed only if:
            // 1. There's another entry at the same location with field info (more specific)
            // 2. And this entry has no meaningful information beyond generic trait bounds

            // Check if there's another entry at the same location with field info
            if entry.field_info.is_none() {
                if let Some(_related_key) = self.find_related_with_field_info(key) {
                    // There's a more specific error at the same location
                    // Suppress this one only if it doesn't have additional useful info
                    let has_useful_delegation_info = !entry.delegation_notes.is_empty()
                        || !entry.provider_relationships.is_empty();

                    if !has_useful_delegation_info {
                        keys_to_suppress.push(key.clone());
                    }
                }
            }
        }

        // Mark suppressed entries
        for key in keys_to_suppress {
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.suppressed = true;
            }
        }
    }

    /// Find a related entry that has field information
    fn find_related_with_field_info(&self, key: &DiagnosticKey) -> Option<DiagnosticKey> {
        for (other_key, other_entry) in &self.entries {
            // Check if locations are the same
            if other_key.location == key.location {
                if other_entry.field_info.is_some() {
                    return Some(other_key.clone());
                }
            }
        }
        None
    }

    /// Get all non-suppressed entries
    pub fn get_active_entries(&self) -> Vec<&DiagnosticEntry> {
        self.entries.values().filter(|e| !e.suppressed).collect()
    }

    /// Get all entries (including suppressed)
    pub fn get_all_entries(&self) -> Vec<&DiagnosticEntry> {
        self.entries.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_database_basic() {
        let db = DiagnosticDatabase::new();
        assert_eq!(db.get_all_entries().len(), 0);
    }
}
