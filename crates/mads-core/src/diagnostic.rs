//! Structured diagnostics and framework errors emitted by MADS.rs.

use std::fmt;

/// A stable identifier for a MADS.rs diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    /// Creates a diagnostic code from a static string.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the string representation of this diagnostic code.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Duplicate provider registration.
pub const MADS001: DiagnosticCode = DiagnosticCode::new("MADS001");

/// Missing provider registration.
pub const MADS003: DiagnosticCode = DiagnosticCode::new("MADS003");

/// Invalid provider configuration.
pub const MADS004: DiagnosticCode = DiagnosticCode::new("MADS004");

/// Route configuration error.
pub const MADS010: DiagnosticCode = DiagnosticCode::new("MADS010");

/// Handler configuration error.
pub const MADS011: DiagnosticCode = DiagnosticCode::new("MADS011");

/// Runtime configuration error.
pub const MADS020: DiagnosticCode = DiagnosticCode::new("MADS020");

/// The source position associated with a diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceLocation {
    /// The source file path.
    pub file: &'static str,
    /// The one-based source line.
    pub line: u32,
    /// The one-based source column.
    pub column: u32,
}

impl SourceLocation {
    /// Creates a source location from a file, line, and column.
    pub const fn new(file: &'static str, line: u32, column: u32) -> Self {
        Self { file, line, column }
    }
}

/// A structured diagnostic with optional source context and suggestions.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    title: String,
    message: String,
    subject: Option<String>,
    location: Option<SourceLocation>,
    suggestions: Vec<String>,
}

impl Diagnostic {
    /// Creates a diagnostic with a code, short title, and detailed message.
    pub fn new(code: DiagnosticCode, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            title: title.into(),
            message: message.into(),
            subject: None,
            location: None,
            suggestions: Vec::new(),
        }
    }

    /// Adds the subject associated with this diagnostic.
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Adds the source location associated with this diagnostic.
    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Adds a remediation suggestion to this diagnostic.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Returns this diagnostic's stable code.
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: {}", self.code, self.title)?;
        if let Some(location) = self.location {
            write!(
                formatter,
                "\n  --> {}:{}:{}",
                location.file, location.line, location.column
            )?;
        }
        if let Some(subject) = &self.subject {
            write!(formatter, "\n  = subject: {subject}")?;
        }
        write!(formatter, "\n  = {}", self.message)?;
        for suggestion in &self.suggestions {
            write!(formatter, "\n  help: {suggestion}")?;
        }
        Ok(())
    }
}

/// A framework error containing a structured diagnostic and optional cause.
#[derive(Debug)]
pub struct Error {
    diagnostic: Diagnostic,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Creates an error from a structured diagnostic.
    pub fn new(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostic,
            source: None,
        }
    }

    /// Creates an error from a structured diagnostic and an underlying cause.
    pub fn with_source<E>(diagnostic: Diagnostic, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            diagnostic,
            source: Some(Box::new(source)),
        }
    }

    /// Returns this error's stable diagnostic code.
    pub const fn code(&self) -> DiagnosticCode {
        self.diagnostic.code()
    }

    /// Returns the structured diagnostic carried by this error.
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

/// The result type used by MADS.rs framework APIs.
pub type Result<T> = std::result::Result<T, Error>;
