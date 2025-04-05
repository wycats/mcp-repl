use nu_protocol::engine::EngineState;
use nu_protocol::engine::StateWorkingSet;

pub mod builtin;
mod call_tool;
pub mod help;
mod list_resources;
mod list_tools;

// Register all custom commands
pub fn register_all(engine_state: &mut EngineState) {
    // Create a working set to register commands
    let mut working_set = StateWorkingSet::new(engine_state);

    // Resources commands
    working_set.add_decl(Box::new(list_resources::ListResourcesCommand {}));

    // Tools commands
    working_set.add_decl(Box::new(list_tools::ListToolsCommand {}));
    working_set.add_decl(Box::new(call_tool::CallToolCommand {}));

    // Render and merge the changes
    let delta = working_set.render();
    if let Err(err) = engine_state.merge_delta(delta) {
        // Use a simpler error handling approach since report_shell_error now requires different args
        eprintln!("Error registering commands: {:?}", err);
    }
}

// Common utilities for commands
pub(crate) mod utils {
    use crate::mcp::McpClient;
    use anyhow::Result;
    use nu_protocol::{Record, Span, Value, engine::Stack};
    use std::sync::Arc;

    // Key used to store the MCP client in the environment variables
    pub const MCP_CLIENT_ENV_VAR: &str = "MCP_CLIENT";

    // Set the MCP client in the stack for access by commands
    pub fn set_mcp_client(stack: &mut Stack, _client: Arc<McpClient>) -> Result<()> {
        // In a real implementation, we should put this in a better place,
        // but for now we'll store it in an environment variable on the stack
        stack.add_env_var(
            MCP_CLIENT_ENV_VAR.to_string(),
            Value::string("mcp-client-present".to_string(), Span::new(0, 0)),
        );
        // Store the actual client in some global place accessible to commands
        // For now, we'll just have a placeholder
        Ok(())
    }

    // Get the MCP client from the stack
    pub fn get_mcp_client(_stack: &Stack) -> Result<Arc<McpClient>, &'static str> {
        // This would normally check if the MCP_CLIENT_ENV_VAR is set
        // and then retrieve the client from some global store
        Err("MCP client not available")
    }

    // Helper to convert string map to Record
    pub fn string_map_to_record(
        map: std::collections::HashMap<String, String>,
        span: Span,
    ) -> Record {
        let mut record = Record::new();

        for (k, v) in map {
            record.push(k, Value::string(v, span));
        }

        record
    }
}
