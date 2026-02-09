use cargo_metadata::Message;

use crate::render_compiler_message::render_compiler_message;

pub fn render_message(message: &Message) {
    match message {
        Message::CompilerMessage(msg) => {
            // Use the rendered field which contains the formatted diagnostic
            if let Ok(rendered) = render_compiler_message(msg) {
                println!("{}", rendered);
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
