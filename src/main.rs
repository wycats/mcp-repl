use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use commands::utils::ReplClient;
use std::env;
use std::sync::Arc;

pub mod commands;
pub mod engine;
pub mod mcp;
pub mod shell;
pub mod util;

#[derive(Parser, Debug)]
#[clap(
    name = "nu-mcp-repl",
    about = "Nushell-based REPL for MCP (Model Context Protocol)"
)]
struct CliArgs {
    #[clap(subcommand)]
    connection: Option<ConnectionType>,

    /// Enable verbose logging
    #[arg(short, long, env = "MCP_VERBOSE")]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum ConnectionType {
    /// Connect to an MCP server via SSE (Server-Sent Events)
    Sse {
        /// URL of the SSE MCP server to connect to
        #[arg(env = "MCP_URL")]
        url: String,
    },

    /// Connect to an MCP server by launching a command
    Command {
        /// Command to launch that implements the MCP protocol
        #[arg(value_parser, env = "MCP_COMMAND")]
        command: String,

        /// Name of the connection
        #[arg(
            value_parser,
            short,
            long,
            required = true,
            env = "MCP_NAME",
            default_value = "mcp"
        )]
        name: String,
    },
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

    println!("Args {:#?}", args);

    if args.verbose {
        println!("Starting MCP REPL in verbose mode");
    }

    // Check environment variables if no connection type is provided
    let connection = args.connection;

    // Set up async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create Tokio runtime")?;

    // Initialize the MCP client with the specified connection type if provided
    let repl_client: Option<Arc<ReplClient>> = if let Some(connection) = connection {
        match connection {
            ConnectionType::Sse { url } => {
                println!("Connecting to MCP server via SSE at: {}", url);
                let connection_type = mcp::McpConnectionType::Sse(url.clone());

                // Connect to the server
                match runtime.block_on(mcp::McpClient::connect(connection_type, args.verbose)) {
                    Ok(client) => {
                        println!("Successfully connected to MCP server via SSE");
                        Some(Arc::new(ReplClient {
                            name: url.clone(),
                            client,
                            debug: args.verbose,
                        }))
                    }
                    Err(err) => {
                        panic!(
                            "Warning: Failed to connect to MCP server ({}): {}",
                            url.clone(),
                            err
                        );
                    }
                }
            }
            ConnectionType::Command { name, command } => {
                println!("Launching MCP server via command: {}", command);
                let connection_type = mcp::McpConnectionType::Command(command.clone());

                // Connect to the server
                match runtime.block_on(mcp::McpClient::connect(connection_type, args.verbose)) {
                    Ok(client) => {
                        println!("Successfully connected to MCP server via command");
                        Some(Arc::new(ReplClient {
                            name,
                            client,
                            debug: args.verbose,
                        }))
                    }
                    Err(err) => {
                        panic!(
                            "Warning: Failed to connect to MCP server ({}): {}",
                            command.clone(),
                            err
                        );
                    }
                }
            }
        }
    } else {
        println!("No MCP server connection specified");
        None
    };

    // Initialize the Nushell-based REPL
    println!("Starting MCP Nushell REPL - Type 'exit' to quit");
    let runtime = Arc::new(runtime);
    let mut repl = shell::McpRepl::new(repl_client, runtime.clone())
        .context("Failed to initialize MCP REPL shell")?;

    // Run the REPL and handle any errors
    match runtime.block_on(repl.run()) {
        Ok(_) => {
            println!("MCP REPL session ended");
            Ok(())
        }
        Err(err) => {
            println!("Error during REPL session: {}", err);
            Err(err)
        }
    }
}
