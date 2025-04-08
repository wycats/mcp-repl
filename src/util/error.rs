use std::ops::Deref;

use nu_protocol::{IntoValue, ShellError, Span, Value};

#[derive(Debug, Clone)]
pub struct McpError(Box<ShellError>);
pub type McpResult<T> = Result<T, McpError>;

impl Deref for McpError {
    type Target = ShellError;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[must_use]
pub fn result_to_val(
    result: McpResult<nu_protocol::Value>,
    span: Option<Span>,
) -> nu_protocol::Value {
    match result {
        Ok(value) => value,
        Err(error) => IntoValue::into_value(error, span.unwrap_or(Span::unknown())),
    }
}

impl IntoValue for McpError {
    fn into_value(self, span: Span) -> Value {
        IntoValue::into_value(*self.0, span)
    }
}

impl From<ShellError> for McpError {
    fn from(error: ShellError) -> Self {
        Self(Box::new(error))
    }
}

impl From<&ShellError> for McpError {
    fn from(error: &ShellError) -> Self {
        Self(Box::new(error.clone()))
    }
}

impl From<&Box<ShellError>> for McpError {
    fn from(error: &Box<ShellError>) -> Self {
        Self(Box::new(error.as_ref().clone()))
    }
}

pub fn generic_error(
    message: impl Into<String>,
    help: impl Into<Option<String>>,
    span: impl Into<Option<Span>>,
) -> McpError {
    McpError(Box::new(ShellError::GenericError {
        error: message.into(),
        msg: String::new(),
        span: span.into(),
        help: help.into(),
        inner: Vec::new(),
    }))
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub enum McpShellError {
    GenericError {
        message: String,
        help: Option<String>,
        span: Option<Span>,
    },
}

impl From<McpShellError> for ShellError {
    fn from(ce: McpShellError) -> Self {
        match ce {
            McpShellError::GenericError {
                message,
                help,
                span,
            } => spanned_shell_error(message, help, span),
        }
    }
}

fn spanned_shell_error(
    msg: impl Into<String>,
    help: impl Into<Option<String>>,
    span: impl Into<Option<Span>>,
) -> ShellError {
    ShellError::GenericError {
        error: msg.into(),
        msg: String::new(),
        span: span.into(),
        help: help.into(),
        inner: Vec::new(),
    }
}
