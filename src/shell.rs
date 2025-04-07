use crate::commands::help::McpHelpCommand;
use crate::config::McpReplConfig;
use crate::engine::get_mcp_client_manager;
use anyhow::{Context, Result};
use log::{debug, info};
use nu_cmd_lang::create_default_context;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{Config, HistoryConfig, HistoryFileFormat, Span, Value};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// Define a static variable to hold our custom history path
static HISTORY_PATH: Lazy<std::sync::Mutex<Option<String>>> =
    Lazy::new(|| std::sync::Mutex::new(None));

// Import Nushell's help commands directly
use crate::commands::builtin::add_shell_command_context;

/// McpRepl integrates Nushell with the MCP functionality
pub struct McpRepl {
    /// Nushell engine state
    engine_state: EngineState,
    /// Nushell stack
    stack: Stack,
}

impl McpRepl {
    /// Create a new MCP REPL instance
    pub fn new() -> Result<Self> {
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

        // Customize history configuration for MCP-REPL
        // Create a separate history file in the .mcp-repl directory
        let history_config = Self::create_custom_history_config()?;
        config.history = history_config;

        // Apply the config
        engine_state.config = Arc::new(config);

        // Mark the engine as interactive to enable features like help
        engine_state.is_interactive = true;

        // Setup a stack with essential environment variables
        let mut stack = Stack::new();

        // Set MCP environment variables if present
        if let Ok(url) = std::env::var("MCP_URL") {
            stack.add_env_var("MCP_URL".to_string(), Value::string(url, Span::unknown()));
        }
        if let Ok(command) = std::env::var("MCP_COMMAND") {
            stack.add_env_var(
                "MCP_COMMAND".to_string(),
                Value::string(command, Span::unknown()),
            );
        }

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

        Ok(Self {
            engine_state,
            stack,
        })
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

    pub async fn register(&mut self, config: &McpReplConfig) -> Result<()> {
        let mut manager = get_mcp_client_manager();

        for (name, server) in &config.servers {
            log::info!("Registering MCP client: {}", name);
            let client = server.to_client(name).await?;
            manager.register_client(name.clone(), client, &mut self.engine_state)?;
        }
        Ok(())
    }

    /// Run the REPL with support for dynamic command registration
    pub fn run(&mut self) -> Result<()> {
        // Run Nushell REPL for one session
        let start_time = Instant::now();
        let repl_result = nu_cli::evaluate_repl(
            &mut self.engine_state,
            self.stack.clone(),
            None, // nushell_path
            None, // load_std_lib
            start_time,
        );

        repl_result.map_err(|e| anyhow::anyhow!("Error during REPL evaluation: {}", e))
    }

    /// Create a custom history configuration for MCP-REPL
    fn create_custom_history_config() -> Result<HistoryConfig> {
        // Create a custom history path in the user's home directory
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;
        let mcp_repl_dir = home_dir.join(".mcp-repl");

        // Create the directory if it doesn't exist
        if !mcp_repl_dir.exists() {
            std::fs::create_dir_all(&mcp_repl_dir)
                .context("Failed to create .mcp-repl directory")?;
        }

        // Use a custom history file
        let history_file = mcp_repl_dir.join("history.txt");
        info!("Using custom history file: {:?}", history_file);

        // Store the path string for later use in the run method
        let history_path_str = history_file.to_string_lossy().to_string();

        // Create a custom history configuration
        let mut history_config = HistoryConfig::default();
        history_config.file_format = HistoryFileFormat::Plaintext;

        // Update the history path in the static
        let mut history_path = HISTORY_PATH.lock().unwrap();
        *history_path = Some(history_path_str);

        Ok(history_config)
    }
}
