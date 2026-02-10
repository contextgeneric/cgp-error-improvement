use cargo_metadata::Message;

use crate::cgp_patterns::is_cgp_diagnostic;
use crate::diagnostic_db::DiagnosticDatabase;

pub fn render_message(message: &Message, db: &mut DiagnosticDatabase) {
    match message {
        Message::CompilerMessage(msg) => {
            // Check if this is a CGP-related error
            if is_cgp_diagnostic(&msg.message) {
                // Add to database for later processing, don't render yet
                db.add_diagnostic(msg);
            } else {
                // Non-CGP error: render immediately using the original rendered field
                if let Some(rendered) = &msg.message.rendered {
                    println!("{}", rendered);
                }
            }
        }
        Message::CompilerArtifact(artifact) => {
            // For now, we'll show the compilation progress
            // Format similar to cargo's output
            let target_name = &artifact.target.name;
            let verb = if artifact.fresh { "Fresh" } else { "Compiling" };
            eprintln!("    {:>12} {}", verb, target_name);
        }
        Message::BuildScriptExecuted(_) => {
            // Silently skip build script notifications for now
        }
        Message::BuildFinished(finished) => {
            if !finished.success {
                eprintln!("Build failed");
            }
        }
        Message::TextLine(_) => {
            // Suppress text lines as they may contain fragments from Cargo's output
            // that interfere with our formatted error messages
        }
        _ => {
            // Ignore any other message types (Message is non-exhaustive)
        }
    }
}
