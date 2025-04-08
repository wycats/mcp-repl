#![deny(missing_docs, unused)]
//! MCP REPL for Nushell
use std::env;

use ::config::{Map, Source, Value};
use anyhow::{Context, Result};
use clap::Parser;
use config::{McpConnectionType, McpReplConfig, parse_env};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub(crate) mod commands;
pub(crate) mod config;
pub(crate) mod engine;
pub(crate) mod mcp;
pub(crate) mod mcp_manager;
pub(crate) mod shell;
pub(crate) mod util;

#[derive(Parser, Debug, Clone, Default)]
#[clap(
    name = "nu-mcp-repl",
    about = "Nushell-based REPL for MCP (Model Context Protocol)"
)]
pub(crate) struct CliArgs {
    /// Enable verbose logging
    #[arg(short, long, env = "MCP_VERBOSE")]
    verbose: bool,

    /// Path to config file
    #[arg(short, long, env = "MCP_CONFIG")]
    config: Option<String>,

    #[command(subcommand)]
    connection: Option<ConnectionType>,
}

/// Type of MCP connection to establish
#[derive(Clone, Debug, Deserialize, Serialize, clap::Parser)]
pub(crate) enum ConnectionType {
    /// SSE-based MCP server (HTTP Server-Sent Events)
    Sse { name: String, url: String },
    /// Command-based MCP server (launches a subprocess)
    Command {
        name: String,
        command: String,
        #[arg(value_parser = parse_env(), long, action = clap::ArgAction::Append)]
        env: Option<IndexMap<String, String>>,
    },
}

fn to_value<'a>(value: &(impl Serialize + Deserialize<'a>)) -> Value {
    let stringify = serde_json::to_string(value).unwrap();
    let value: Value = serde_json::from_str(&stringify).unwrap();
    value
}

impl Source for CliArgs {
    fn collect(&self) -> ::std::result::Result<Map<String, Value>, ::config::ConfigError> {
        let mut servers: Map<String, Value> = ::config::Map::new();
        if let Some(connection) = &self.connection {
            // first, create a `ServerConfig`
            match connection {
                ConnectionType::Sse { name, url } => {
                    servers.insert(
                        name.to_string(),
                        to_value(&McpConnectionType::Sse {
                            url: url.to_string(),
                        }),
                    );
                }
                ConnectionType::Command { name, command, env } => {
                    servers.insert(
                        name.to_string(),
                        to_value(&McpConnectionType::Command {
                            command: command.to_string(),
                            env: env.clone(),
                        }),
                    );
                }
            }

            let mut map = Map::new();
            map.insert("servers".to_string(), Value::from(servers));
            return Ok(map);
        }

        Ok(Map::new())
    }

    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }
}

fn main() -> Result<()> {
    // Initialize logging with filter for prompt warnings
    let default_level = if env::var("RUST_LOG").is_ok() {
        "info"
    } else {
        "warn"
    };

    env_logger::Builder::from_env(env_logger::Env::default().filter_or("RUST_LOG", default_level))
        .filter_module("nu_cli::prompt_update", log::LevelFilter::Error)
        .init();

    // Parse command line arguments
    let args = CliArgs::parse();
    let config = McpReplConfig::env(&args).context("Failed to load configuration")?;

    log::trace!("Args {args:#?}");

    if args.verbose {
        log::info!("Starting MCP REPL in verbose mode");
    }

    // Initialize the Nushell-based REPL
    log::info!("Starting MCP Nushell REPL - Type 'exit' to quit");
    let mut repl = shell::McpRepl::new().context("Failed to initialize MCP REPL shell")?;

    let rt = tokio::runtime::Runtime::new().context("Failed to create runtime")?;

    rt.block_on(repl.register(&config))
        .context("Failed to register MCP clients")?;

    // Run the REPL and handle any errors
    match repl.run() {
        Ok(()) => {
            log::debug!("MCP REPL session ended");
            Ok(())
        }
        Err(err) => {
            log::error!("Error during REPL session: {err}");
            Err(err)
        }
    }
}
