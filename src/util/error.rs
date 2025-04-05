use nu_protocol::{ShellError, Span};

pub fn generic_error(
    message: impl Into<String>,
    help: impl Into<Option<String>>,
    span: impl Into<Option<Span>>,
) -> ShellError {
    McpShellError::GenericError {
        message: message.into(),
        help: help.into(),
        span: span.into(),
    }
    .into()
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
        msg: "".to_string(),
        span: span.into(),
        help: help.into(),
        inner: Vec::new(),
    }
}
