use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::sync::Arc;

mod commands;
mod config;
mod mcp;
mod shell;
mod util;

#[derive(Parser, Debug)]
#[clap(
    name = "nu-mcp-repl",
    about = "Nushell-based REPL for MCP (Model Context Protocol)"
)]
struct CliArgs {
    #[clap(subcommand)]
    connection: Option<ConnectionType>,

    /// Enable verbose logging
    #[clap(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum ConnectionType {
    /// Connect to an MCP server via SSE (Server-Sent Events)
    Sse {
        /// URL of the SSE MCP server to connect to
        url: String,
    },

    /// Connect to an MCP server by launching a command
    Command {
        /// Command to launch that implements the MCP protocol
        #[clap(value_parser)]
        command: String,
    },
}

fn main() -> Result<()> {
    // Initialize logging based on verbosity flag or RUST_LOG env var
    let env = env_logger::Env::default().filter_or(
        "RUST_LOG",
        if env::var("RUST_LOG").is_ok() {
            "info"
        } else {
            "warn"
        },
    );
    env_logger::init_from_env(env);

    // Parse command line arguments
    let args = CliArgs::parse();

    if args.verbose {
        println!("Starting MCP REPL in verbose mode");
    }

    // Set up async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create Tokio runtime")?;

    // Run the REPL
    runtime.block_on(async {
        // Initialize the MCP client with the specified connection type if provided
        let mcp_client = if let Some(connection) = &args.connection {
            match connection {
                ConnectionType::Sse { url } => {
                    println!("Connecting to MCP server via SSE at: {}", url);
                    let connection_type = mcp::McpConnectionType::Sse(url.clone());

                    // Connect to the server
                    match mcp::McpClient::connect(connection_type).await {
                        Ok(client) => {
                            println!("Successfully connected to MCP server via SSE");
                            Some(Arc::new(client))
                        }
                        Err(err) => {
                            println!("Warning: Failed to connect to MCP server: {}", err);
                            None
                        }
                    }
                }
                ConnectionType::Command { command } => {
                    println!("Launching MCP server via command: {}", command);
                    let connection_type = mcp::McpConnectionType::Command(command.clone());

                    // Connect to the server
                    match mcp::McpClient::connect(connection_type).await {
                        Ok(client) => {
                            println!("Successfully connected to MCP server via command");
                            Some(Arc::new(client))
                        }
                        Err(err) => {
                            println!("Warning: Failed to connect to MCP server: {}", err);
                            None
                        }
                    }
                }
            }
        } else {
            println!("No MCP server connection specified. Use 'sse' or 'command' subcommand.");
            println!("You can still run the shell, but MCP commands will not be available.");
            None
        };

        // Initialize the Nushell-based REPL
        println!("Starting MCP Nushell REPL - Type 'exit' to quit");
        let mut repl =
            shell::McpRepl::new(mcp_client).context("Failed to initialize MCP REPL shell")?;

        // Run the REPL and handle any errors
        match repl.run().await {
            Ok(_) => {
                println!("MCP REPL session ended");
                Ok(())
            }
            Err(e) => {
                eprintln!("Error in MCP REPL session: {}", e);
                Err(e.into())
            }
        }
    })
}
