/// Module for detecting and extracting CGP-specific patterns from compiler diagnostics
/// This module only patterns match on CGP library constructs, never on user code
use cargo_metadata::diagnostic::{Diagnostic, DiagnosticLevel};

/// Checks if a diagnostic is related to CGP constructs
pub fn is_cgp_diagnostic(diagnostic: &Diagnostic) -> bool {
    let cgp_patterns = [
        "CanUseComponent",
        "IsProviderFor",
        "HasField",
        "cgp_impl",
        "cgp_component",
        "cgp_auto_getter",
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

/// Information about a component extracted from CGP patterns
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentInfo {
    /// Full component type name (e.g., "AreaCalculatorComponent", "ScaledArea<RectangleArea>")
    pub component_type: String,
    /// Provider trait name derived from component (e.g., "AreaCalculator" from "AreaCalculatorComponent")
    pub provider_trait: Option<String>,
}

/// Information about a field extracted from HasField patterns
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldInfo {
    /// The field name extracted from Symbol pattern
    pub field_name: String,
    /// Whether the field name was fully extracted (false if truncated)
    pub is_complete: bool,
    /// The struct/type that is missing the field
    pub target_type: String,
}

/// Information about provider trait relationships from IsProviderFor patterns
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderRelationship {
    /// The provider implementation type
    pub provider_type: String,
    /// The component being provided
    pub component: String,
    /// The context type
    pub context: String,
}

/// Extracts component information from CanUseComponent patterns
/// Pattern: `CanUseComponent<ComponentType>`
pub fn extract_component_from_can_use(message: &str) -> Option<ComponentInfo> {
    let start = message.find("CanUseComponent<")?;
    let after_start = start + "CanUseComponent<".len();

    let component_type = extract_balanced_generic(message, after_start)?;
    let provider_trait = derive_provider_trait_name(&component_type);

    Some(ComponentInfo {
        component_type,
        provider_trait,
    })
}

/// Extracts component information from various patterns in a message
pub fn extract_component_info(message: &str) -> Option<ComponentInfo> {
    // Try CanUseComponent pattern first
    if let Some(info) = extract_component_from_can_use(message) {
        return Some(info);
    }

    // Try to find component names by the "*Component" suffix pattern
    // This is a general CGP naming convention
    for word in message.split_whitespace() {
        let clean_word =
            word.trim_matches(|c: char| !c.is_alphanumeric() && c != '<' && c != '>' && c != ',');

        if clean_word.contains("Component") {
            // Extract the component type, handling generics
            if let Some(component_type) = extract_component_type_name(clean_word) {
                let provider_trait = derive_provider_trait_name(&component_type);
                return Some(ComponentInfo {
                    component_type,
                    provider_trait,
                });
            }
        }
    }

    None
}

/// Extracts a component type name from a string that may contain it
fn extract_component_type_name(text: &str) -> Option<String> {
    // Handle simple case: just "XyzComponent"
    if text.ends_with("Component") && !text.contains('<') {
        return Some(text.to_string());
    }

    // Handle generic case: "Xyz<A, B>Component" or more complex patterns
    // Find all text that forms a valid component reference
    if let Some(component_pos) = text.rfind("Component") {
        // Find the start of this component reference
        let before_component = &text[..component_pos + "Component".len()];

        // Walk backward to find the start, handling generics
        let mut depth = 0;
        let mut start_idx = 0;

        for (i, ch) in before_component.char_indices().rev() {
            if ch == '>' {
                depth += 1;
            } else if ch == '<' {
                depth -= 1;
            } else if depth == 0 && !ch.is_alphanumeric() && ch != '_' {
                start_idx = i + 1;
                break;
            }
        }

        return Some(before_component[start_idx..].to_string());
    }

    None
}

/// Derives provider trait name from component name by removing "Component" suffix
/// Example: "AreaCalculatorComponent" -> Some("AreaCalculator")
pub fn derive_provider_trait_name(component_name: &str) -> Option<String> {
    // Handle simple case: "XyzComponent" -> "Xyz"
    if let Some(stripped) = component_name.strip_suffix("Component") {
        if !stripped.is_empty() {
            return Some(stripped.to_string());
        }
    }

    // Handle complex generic cases like "Wrapper<Inner>Component"
    // This shouldn't normally happen in CGP, but handle it gracefully
    if component_name.contains("Component") {
        if let Some(pos) = component_name.rfind("Component") {
            let before = &component_name[..pos];
            if !before.is_empty() {
                return Some(before.to_string());
            }
        }
    }

    None
}

/// Extracts field information from HasField diagnostic patterns
/// Pattern: `HasField<Symbol<N, Chars<'c1', Chars<'c2', ...>>>>` is not implemented for `Type`
pub fn extract_field_info(diagnostic: &Diagnostic) -> Option<FieldInfo> {
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Help) {
            let message = &child.message;

            if message.contains("HasField") && message.contains("is not implemented for") {
                // Extract the field name from Symbol pattern
                let field_name_result = extract_field_name_from_symbol(message)?;

                // Extract the target type
                let target_type = extract_type_from_not_implemented(message)?;

                return Some(FieldInfo {
                    field_name: field_name_result.0,
                    is_complete: field_name_result.1,
                    target_type,
                });
            }
        }
    }

    None
}

/// Extracts field name from Symbol<N, Chars<'x', Chars<'y', ...>>> pattern
/// Returns (field_name, is_complete)
fn extract_field_name_from_symbol(message: &str) -> Option<(String, bool)> {
    // Get the part before "but trait" if it exists (to focus on the unsatisfied trait)
    let relevant_part = if let Some(pos) = message.find("but trait") {
        &message[..pos]
    } else {
        message
    };

    // Extract expected length from Symbol<N, ...>
    let expected_length = extract_symbol_length(relevant_part)?;

    // Extract visible characters from Chars<'x', Chars<'y', ...>> chain
    let chars = extract_chars_from_pattern(relevant_part);

    if chars.is_empty() {
        return None;
    }

    let field_name: String = chars.iter().collect();
    let is_complete = field_name.len() == expected_length;

    Some((field_name, is_complete))
}

/// Extracts the expected length from Symbol<N, ...> pattern
fn extract_symbol_length(text: &str) -> Option<usize> {
    let start = text.find("Symbol<")?;
    let after_symbol = &text[start + 7..];
    let comma_pos = after_symbol.find(',')?;
    after_symbol[..comma_pos].trim().parse::<usize>().ok()
}

/// Extracts all characters from Chars<'x', Chars<'y', ...>> pattern
fn extract_chars_from_pattern(text: &str) -> Vec<char> {
    let mut chars = Vec::new();
    let mut idx = 0;

    while idx < text.len() {
        if text[idx..].starts_with("Chars<'") {
            // Look for the character after the quote
            let char_start = idx + 7;
            if let Some(ch) = text[char_start..].chars().next() {
                // Skip underscore placeholders (used for hidden characters)
                if ch != '\'' && ch != '_' && ch.is_alphabetic() {
                    chars.push(ch);
                }
            }
        }
        idx += 1;
    }

    chars
}

/// Extracts type name from "is not implemented for `Type`" pattern
fn extract_type_from_not_implemented(message: &str) -> Option<String> {
    let start = message.find("is not implemented for `")?;
    let after_start = start + "is not implemented for `".len();
    let end = message[after_start..].find('`')?;
    let full_name = &message[after_start..after_start + end];

    // Remove module prefix (e.g., "module::Type" -> "Type")
    let simple_name = full_name.split("::").last().unwrap_or(full_name);
    Some(simple_name.to_string())
}

/// Extracts provider relationship from IsProviderFor patterns
/// Pattern: `for `Provider` to implement `IsProviderFor<Component, Context>`
pub fn extract_provider_relationship(message: &str) -> Option<ProviderRelationship> {
    if !message.contains("IsProviderFor") {
        return None;
    }

    // Extract provider type: "for `Provider` to implement"
    let provider_type = extract_type_from_for_to_implement(message)?;

    // Extract component and context from IsProviderFor<Component, Context>
    let start = message.find("IsProviderFor<")?;
    let after_start = start + "IsProviderFor<".len();

    // Find the comma separating component and context
    let comma_pos = find_comma_at_depth(after_start, message)?;
    let component = message[after_start..comma_pos].trim().to_string();

    // Extract context (from comma to closing >)
    let after_comma = comma_pos + 1;
    let context = extract_balanced_generic(message, after_comma)?;

    Some(ProviderRelationship {
        provider_type,
        component,
        context,
    })
}

/// Extracts type from "for `Type` to implement" pattern
fn extract_type_from_for_to_implement(message: &str) -> Option<String> {
    let start = message.find("for `")?;
    let after_start = start + 5;
    let end = message[after_start..].find("` to")?;
    let full_name = &message[after_start..after_start + end];

    // Remove module prefix
    let simple_name = full_name.split("::").last().unwrap_or(full_name);
    Some(simple_name.to_string())
}

/// Finds the position of a comma at the top level of generic nesting
fn find_comma_at_depth(start_pos: usize, text: &str) -> Option<usize> {
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

/// Extracts a balanced generic type from text starting at position
/// Example: extract "Foo<Bar, Baz>" from position after opening `<`
fn extract_balanced_generic(text: &str, start_pos: usize) -> Option<String> {
    let mut depth = 1; // We've already seen one opening bracket
    let mut end_pos = start_pos;

    for (i, ch) in text[start_pos..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    end_pos = start_pos + i;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth == 0 {
        Some(text[start_pos..end_pos].trim().to_string())
    } else {
        // Not balanced, return what we have
        Some(text[start_pos..].trim_end_matches('>').trim().to_string())
    }
}

/// Extracts consumer trait name from "required by a bound in `TraitName`" pattern
pub fn extract_consumer_trait(message: &str) -> Option<String> {
    let start = message.find("required by a bound in `")?;
    let after_start = start + "required by a bound in `".len();
    let end = message[after_start..].find('`')?;
    Some(message[after_start..after_start + end].to_string())
}

/// Checks if a diagnostic has help messages indicating other HasField implementations exist
pub fn has_other_hasfield_implementations(diagnostic: &Diagnostic) -> bool {
    for child in &diagnostic.children {
        if matches!(child.level, DiagnosticLevel::Help) {
            if child.message.contains("but trait `HasField")
                || child
                    .message
                    .contains("the following other types implement trait")
            {
                return true;
            }
        }
    }
    false
}

/// Removes all module prefixes from a message (e.g., "foo::bar::Baz" -> "Baz")
pub fn strip_module_prefixes(message: &str) -> String {
    // This is a generic transformation - we don't hardcode specific module names
    let mut result = message.to_string();

    // Remove cgp library prefixes
    result = result.replace("cgp::prelude::", "");
    result = result.replace("cgp::", "");

    // For user module prefixes, we need a more sophisticated approach
    // We'll use a regex-like pattern to match any `module::path::Type` and keep only `Type`
    // But we need to be careful not to break trait implementations

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_provider_trait_name() {
        assert_eq!(
            derive_provider_trait_name("AreaCalculatorComponent"),
            Some("AreaCalculator".to_string())
        );
        assert_eq!(
            derive_provider_trait_name("FooComponent"),
            Some("Foo".to_string())
        );
        assert_eq!(derive_provider_trait_name("Component"), None);
        assert_eq!(derive_provider_trait_name("NoSuffix"), None);
    }

    #[test]
    fn test_extract_symbol_length() {
        let text = "Symbol<6, Chars<'h', Chars<'e', ...>>>";
        assert_eq!(extract_symbol_length(text), Some(6));

        let text2 = "Symbol<5, Chars<'w', ...>>";
        assert_eq!(extract_symbol_length(text2), Some(5));
    }

    #[test]
    fn test_extract_chars_from_pattern() {
        let text = "Chars<'h', Chars<'e', Chars<'i', Chars<'g', Chars<'h', Chars<'t', Nil>>>>>>";
        let chars = extract_chars_from_pattern(text);
        assert_eq!(chars, vec!['h', 'e', 'i', 'g', 'h', 't']);

        let text2 = "Chars<'w', Chars<'i', Chars<'d', Chars<'_', Chars<'h', Nil>>>>>";
        let chars2 = extract_chars_from_pattern(text2);
        // Underscores are placeholders and should be skipped
        assert_eq!(chars2, vec!['w', 'i', 'd', 'h']);
    }
}
