/// Main orchestrator for rendering compiler messages with CGP-aware improvements
/// This module has been refactored to use the modular architecture described in the report
use anyhow::Error;
use cargo_metadata::CompilerMessage;

use crate::cgp_patterns::is_cgp_diagnostic;
use crate::diagnostic_db::DiagnosticDatabase;
use crate::error_formatting::format_error_message;

/// Main entry point for rendering a compiler message
/// This function processes individual messages, but for best results,
/// use render_compiler_messages() to process multiple related messages together
pub fn render_compiler_message(message: &CompilerMessage) -> Result<String, Error> {
    let diagnostic = &message.message;

    // Check if this is a CGP-related error
    if is_cgp_diagnostic(diagnostic) {
        // For single message processing, create a temporary database
        let mut db = DiagnosticDatabase::new();
        db.add_diagnostic(diagnostic);
        db.deduplicate();

        let entries = db.get_active_entries();
        if entries.is_empty() {
            // All entries were suppressed
            Ok(String::new())
        } else {
            // Format the first active entry
            Ok(format_error_message(entries[0]))
        }
    } else {
        // Return the original rendered message for non-CGP errors
        if let Some(rendered) = &diagnostic.rendered {
            Ok(rendered.clone())
        } else {
            Ok(String::new())
        }
    }
}

/// Renders multiple compiler messages together (better for related errors)
/// This allows proper merging and deduplication of related diagnostics
pub fn render_compiler_messages(messages: &[CompilerMessage]) -> Result<Vec<String>, Error> {
    // Build database from all messages
    let mut db = DiagnosticDatabase::new();

    for message in messages {
        let diagnostic = &message.message;
        if is_cgp_diagnostic(diagnostic) {
            db.add_diagnostic(diagnostic);
        }
    }

    // Apply deduplication
    db.deduplicate();

    // Format all active entries
    let mut results = Vec::new();
    for entry in db.get_active_entries() {
        let formatted = format_error_message(entry);
        if !formatted.is_empty() {
            results.push(formatted);
        }
    }

    Ok(results)
}
