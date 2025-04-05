use crate::commands::help::McpHelpCommand;
use crate::mcp::McpClient;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use nu_cmd_lang::create_default_context;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{Config, Span, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// Import Nushell's help commands directly
use crate::commands::builtin::add_shell_command_context;

/// McpRepl integrates Nushell with the MCP functionality
pub struct McpRepl {
    /// Nushell engine state
    engine_state: EngineState,
    /// Nushell stack
    stack: Stack,
    /// MCP client (if available)
    mcp_client: Option<Arc<McpClient>>,
}

impl McpRepl {
    /// Create a new MCP REPL instance
    pub fn new(mcp_client: Option<Arc<McpClient>>) -> Result<Self> {
        // Initialize a clean Nushell engine with default commands
        let mut engine_state = create_default_context();

        // Create a minimalist configuration
        let mut config = Config::default();
        config.show_banner = Value::bool(false, Span::unknown());

        // Initialize hooks with empty values - don't set to None
        config.hooks.display_output = None;
        config.hooks.command_not_found = None;
        config.hooks.env_change = HashMap::new();
        config.hooks.pre_prompt = Vec::new();
        config.hooks.pre_execution = Vec::new();

        // Apply the config
        engine_state.config = Arc::new(config);

        // Mark the engine as interactive to enable features like help
        engine_state.is_interactive = true;

        // Setup a stack with essential environment variables
        let mut stack = Stack::new();

        // Set up minimal environment variables required for commands to work
        stack.add_env_var("PWD".into(), Value::string("/", Span::unknown()));

        // Add PROMPT_COMMAND to display a simple prompt
        stack.add_env_var(
            "PROMPT_COMMAND".into(),
            Value::string("> ", Span::unknown()),
        );

        // Ensure an exit code is set
        stack.set_last_exit_code(0, Span::unknown());

        // Add command duration placeholder (used by some commands)
        stack.add_env_var(
            "CMD_DURATION_MS".into(),
            Value::string("0", Span::unknown()),
        );

        info!("Initialized minimal Nushell engine state");

        // Register custom MCP commands
        Self::register_mcp_commands(&mut engine_state);
        debug!("Registered MCP commands in engine state");

        // Log if MCP client is available
        if let Some(client) = &mcp_client {
            // We'll add client access to commands through the command context
            let server_info: String = client
                .server_info()
                .context("Failed to get MCP server info")?;
            info!("MCP client initialized - connected to {}", server_info);
            println!("Connected to MCP server: {}", server_info);
        } else {
            warn!("No MCP client available - some commands will not work");
        }

        Ok(Self {
            engine_state,
            stack,
            mcp_client,
        })
    }

    /// Set the MCP client for this REPL
    pub fn with_mcp_client(mut self, client: Arc<McpClient>) -> Self {
        self.mcp_client = Some(client);
        self
    }

    /// Register MCP-specific Nushell commands and essential Nushell commands
    fn register_mcp_commands(engine_state: &mut EngineState) {
        // Register custom commands from our commands module
        crate::commands::register_all(engine_state);

        // Add shell command context (without system/os commands)
        // This function takes ownership of engine_state and returns a new one
        *engine_state = add_shell_command_context(engine_state.clone());

        // Initialize environment variables in both engine_state and the Nushell config
        let mut env_vars = std::env::vars().collect::<Vec<_>>();
        env_vars.sort_by(|a, b| a.0.cmp(&b.0)); // Sort for predictable order

        // First, make sure PATH is available (critical for command execution)
        // If not in the environment, use a reasonable default
        let path = std::env::var("PATH").unwrap_or_else(|_| {
            // Fallback to common paths if PATH isn't set
            "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
        });
        engine_state.add_env_var("PATH".to_string(), Value::string(path, Span::unknown()));

        // Add all other environment variables from the host system
        for (key, val) in env_vars {
            // Skip PATH since we already handled it specially
            if key != "PATH" {
                engine_state.add_env_var(key, Value::string(val, Span::unknown()));
            }
        }

        // Make sure critical Nushell variables are set
        // Current directory
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(cwd_str) = cwd.to_str() {
                engine_state
                    .add_env_var("PWD".to_string(), Value::string(cwd_str, Span::unknown()));
            }
        }

        // OLDPWD (used by cd and other commands)
        engine_state.add_env_var("OLDPWD".to_string(), Value::string("", Span::unknown()));

        // Exit code of last command
        engine_state.add_env_var("LAST_EXIT_CODE".to_string(), Value::int(0, Span::unknown()));

        let mut working_set = StateWorkingSet::new(engine_state);
        working_set.add_decl(Box::new(McpHelpCommand));
        let delta = working_set.render();
        if let Err(err) = engine_state.merge_delta(delta) {
            log::warn!("Error registering custom help command: {:?}", err);
        }
    }

    /// Run the REPL
    pub async fn run(&mut self) -> Result<()> {
        // Make MCP client available to commands
        self.setup_mcp_client()
            .context("Failed to set up MCP client in REPL context")?;
        debug!("Set up MCP client in REPL context");

        // Use Nushell's built-in REPL evaluation
        let start_time = Instant::now();
        info!("Starting REPL evaluation");

        // Skip reading any config files - we want a completely isolated environment

        // Current signature of evaluate_repl in nu-cli 0.93.0+
        nu_cli::evaluate_repl(
            &mut self.engine_state,
            self.stack.clone(),
            None, // nushell_path
            None, // load_std_lib
            start_time,
        )
        .expect("Error during REPL evaluation");

        Ok(())
    }

    /// Set up the MCP client in the engine state
    /// This allows commands to access the MCP client
    fn setup_mcp_client(&mut self) -> Result<()> {
        if let Some(mcp_client) = &self.mcp_client {
            // Store the MCP client in the stack for command access
            if let Err(e) =
                crate::commands::utils::set_mcp_client(&mut self.stack, mcp_client.clone())
            {
                eprintln!("Warning: Failed to set MCP client in stack: {}", e);
            }
        }
        Ok(())
    }

    /// Get a reference to the MCP client
    pub fn mcp_client(&self) -> Option<&Arc<McpClient>> {
        self.mcp_client.as_ref()
    }
}
