use cargo_metadata::Message;

use crate::cgp_patterns::is_cgp_diagnostic;
use crate::diagnostic_db::DiagnosticDatabase;

/// Output mode for rendering messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Human-readable mode: print non-CGP messages immediately
    Human,
    /// JSON mode: collect all messages for later output
    Json,
}

/// Render a message in human-readable mode (prints non-CGP messages immediately)
pub fn render_message(message: &Message, db: &mut DiagnosticDatabase) {
    let _ = render_message_with_mode(message, db, RenderMode::Human);
}

/// Render a message with specified mode
pub fn render_message_with_mode(
    message: &Message,
    db: &mut DiagnosticDatabase,
    mode: RenderMode,
) -> Option<Message> {
    match message {
        Message::CompilerMessage(msg) => {
            // Check if this is a CGP-related error
            if is_cgp_diagnostic(&msg.message) {
                // Add to database for later processing
                db.add_diagnostic(msg);
                None
            } else {
                // Non-CGP error
                match mode {
                    RenderMode::Human => {
                        // Render immediately using the original rendered field
                        if let Some(rendered) = &msg.message.rendered {
                            println!("{}", rendered);
                        }
                        None
                    }
                    RenderMode::Json => {
                        // Return the message for later JSON output
                        Some(message.clone())
                    }
                }
            }
        }
        Message::CompilerArtifact(artifact) => {
            match mode {
                RenderMode::Human => {
                    // For human mode, show the compilation progress
                    let target_name = &artifact.target.name;
                    let verb = if artifact.fresh { "Fresh" } else { "Compiling" };
                    eprintln!("    {:>12} {}", verb, target_name);
                    None
                }
                RenderMode::Json => Some(message.clone()),
            }
        }
        Message::BuildScriptExecuted(_) => {
            match mode {
                RenderMode::Human => None, // Silently skip
                RenderMode::Json => Some(message.clone()),
            }
        }
        Message::BuildFinished(finished) => match mode {
            RenderMode::Human => {
                if !finished.success {
                    eprintln!("Build failed");
                }
                None
            }
            RenderMode::Json => Some(message.clone()),
        },
        Message::TextLine(_) => {
            match mode {
                RenderMode::Human => None, // Suppress text lines
                RenderMode::Json => Some(message.clone()),
            }
        }
        _ => {
            // Ignore any other message types in human mode
            // In JSON mode, pass them through
            match mode {
                RenderMode::Human => None,
                RenderMode::Json => Some(message.clone()),
            }
        }
    }
}
