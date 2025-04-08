#![allow(dead_code)]
//! Status message utilities for the MCP REPL
//! Provides pretty-formatted status messages that stand out from regular logging

use std::io::{self, Write};

use nu_ansi_term;
use nu_color_config::StyleComputer;
use nu_protocol::{Span, Value};

/// Level of status message
#[derive(Debug, Clone, Copy)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

/// Print an info status message
#[macro_export]
macro_rules! info {
    ($msg:expr) => {
        $crate::util::status::print_status(&format!($msg), "INFO", $crate::util::status::Level::Info)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::util::status::print_status(&format!($fmt, $($arg)*), "INFO", $crate::util::status::Level::Info)
    };
}

/// Print a success status message
#[macro_export]
macro_rules! success {
    ($msg:expr) => {
        $crate::util::status::print_status($msg, "SUCCESS", $crate::util::status::Level::Success)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::util::status::print_status(&format!($fmt, $($arg)*), "SUCCESS", $crate::util::status::Level::Success)
    };
}

/// Print a warning status message
#[macro_export]
macro_rules! warning {
    ($msg:expr) => {
        $crate::util::status::print_status($msg, "WARNING", $crate::util::status::Level::Warning)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::util::status::print_status(&format!($fmt, $($arg)*), "WARNING", $crate::util::status::Level::Warning)
    };
}

/// Print an error status message
#[macro_export]
macro_rules! error {
    ($msg:expr) => {
        $crate::util::status::print_status($msg, "ERROR", $crate::util::status::Level::Error)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::util::status::print_status(&format!($fmt, $($arg)*), "ERROR", $crate::util::status::Level::Error)
    };
}

/// Internal implementation for all status messages
pub fn print_status(message: &str, prefix: &str, level: Level) {
    let span = Span::unknown();
    // We need to create a mock engine state and stack since we're not in a command context
    let engine_state = nu_protocol::engine::EngineState::new();
    let stack = nu_protocol::engine::Stack::new();
    let style_computer = StyleComputer::from_config(&engine_state, &stack);

    // Create a value to style
    let prefix_value = Value::string(format!("[{prefix}]"), span);

    // Style based on level - using Nushell's built-in style names
    let style = match level {
        Level::Info => style_computer.compute("header", &prefix_value),
        Level::Success => style_computer.compute("string", &prefix_value).bold(),
        Level::Warning => nu_ansi_term::Style::new()
            .fg(nu_ansi_term::Color::Yellow)
            .bold(),
        Level::Error => nu_ansi_term::Style::new()
            .fg(nu_ansi_term::Color::Red)
            .bold(),
    };

    // Apply the style to the prefix text
    let styled_prefix = style.paint(format!("[{prefix}]"));

    // Print to stdout (no log noise)
    let _ = io::stdout().write_all(format!("{styled_prefix} {message}\n").as_bytes());
}
