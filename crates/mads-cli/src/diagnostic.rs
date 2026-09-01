use std::{error::Error, fmt};

pub(crate) const MADS200: &str = "MADS200";
pub(crate) const MADS201: &str = "MADS201";
pub(crate) const MADS202: &str = "MADS202";
pub(crate) const MADS220: &str = "MADS220";

pub(crate) struct CliError {
    code: &'static str,
    title: &'static str,
    message: String,
    subject: Option<String>,
    suggestions: Vec<String>,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl CliError {
    pub(crate) fn new(code: &'static str, title: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            title,
            message: message.into(),
            subject: None,
            suggestions: Vec::new(),
            source: None,
        }
    }

    pub(crate) fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub(crate) fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    pub(crate) fn with_source<E>(mut self, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: {}", self.code, self.title)?;
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

impl fmt::Debug for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = if self.source.is_some() {
            "[REDACTED]"
        } else {
            self.message.as_str()
        };
        let source = if self.source.is_some() {
            "[REDACTED]"
        } else {
            "None"
        };

        formatter
            .debug_struct("CliError")
            .field("code", &self.code)
            .field("title", &self.title)
            .field("message", &message)
            .field("subject", &self.subject)
            .field("suggestions", &self.suggestions)
            .field("source", &source)
            .finish()
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{CliError, MADS200, MADS201};

    #[test]
    fn renders_a_stable_human_diagnostic() {
        let error = CliError::new(
            MADS200,
            "Cargo application target is ambiguous",
            "more than one package can be selected",
        )
        .with_subject("workspace")
        .with_suggestion("pass --package <package>");

        assert_eq!(
            error.to_string(),
            "error[MADS200]: Cargo application target is ambiguous\n  = subject: workspace\n  = more than one package can be selected\n  help: pass --package <package>"
        );
    }

    #[test]
    fn redacts_a_sourced_diagnostic_from_debug_output() {
        let error = CliError::new(MADS201, "Cargo metadata failed", "/absolute/secret")
            .with_source(io::Error::other("/absolute/registry/path"));

        let debug = format!("{error:?}");
        assert!(debug.contains(MADS201));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("/absolute"));
    }
}
