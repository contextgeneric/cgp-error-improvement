use anyhow::Error;
use cargo_metadata::CompilerMessage;

pub fn render_compiler_message(message: &CompilerMessage) -> Result<String, Error> {
    if let Some(rendered) = &message.message.rendered {
        // TODO: format CGP error messages differently
        Ok(rendered.clone())
    } else {
        Ok("".to_owned())
    }
}