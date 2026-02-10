/// Main orchestrator for rendering compiler messages with CGP-aware improvements
/// This module has been refactored to use the modular architecture described in the report
use anyhow::Error;
use cargo_metadata::CompilerMessage;

use crate::cgp_patterns::is_cgp_diagnostic;
use crate::diagnostic_db::DiagnosticDatabase;

/// Main entry point for rendering a compiler message
/// This function processes individual messages, but for best results,
/// use render_compiler_messages() to process multiple related messages together
pub fn render_compiler_message(message: &CompilerMessage) -> Result<String, Error> {
    let diagnostic = &message.message;

    // Check if this is a CGP-related error
    if is_cgp_diagnostic(diagnostic) {
        // For single message processing, create a temporary database
        let mut db = DiagnosticDatabase::new();
        db.add_diagnostic(message);

        let rendered_messages = db.render_compiler_messages();
        if rendered_messages.is_empty() {
            // All entries were suppressed
            Ok(String::new())
        } else {
            // Return the rendered field of the first message
            Ok(rendered_messages[0]
                .message
                .rendered
                .clone()
                .unwrap_or_default())
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
            db.add_diagnostic(message);
        }
    }

    // Get rendered messages
    let rendered_messages = db.render_compiler_messages();

    // Extract the rendered strings
    let results = rendered_messages
        .into_iter()
        .filter_map(|msg| msg.message.rendered)
        .collect();

    Ok(results)
}
