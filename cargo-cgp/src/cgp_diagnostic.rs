use miette::{Diagnostic, LabeledSpan, NamedSource};
use std::fmt;

/// A CGP-aware diagnostic that implements miette's Diagnostic trait
#[derive(Debug, Clone)]
pub struct CgpDiagnostic {
    /// The main error message
    pub message: String,
    /// Error code (e.g., "E0277")
    pub code: Option<String>,
    /// Help text with suggestions
    pub help: Option<String>,
    /// Source code with file name
    pub source_code: Option<NamedSource<String>>,
    /// Labeled spans for highlighting
    pub labels: Vec<LabeledSpan>,
}

impl fmt::Display for CgpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CgpDiagnostic {}

impl Diagnostic for CgpDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.code
            .as_ref()
            .map(|c| Box::new(c.clone()) as Box<dyn fmt::Display>)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|h| Box::new(h.clone()) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code
            .as_ref()
            .map(|s| s as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        if self.labels.is_empty() {
            None
        } else {
            Some(Box::new(self.labels.clone().into_iter()))
        }
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        None // We'll add related diagnostics through help/notes text
    }
}
