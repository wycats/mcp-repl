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
    use crate::engine::EngineStateExt;
    use crate::mcp::McpClient;
    use nu_protocol::engine::EngineState;
    use nu_protocol::{Record, Span, Value};
    use std::sync::Arc;

    // Set the MCP client for access by commands
    pub fn set_mcp_client(engine_state: &mut EngineState, client: Arc<McpClient>) {
        engine_state.set_mcp_client(client);
    }

    // Get the MCP client
    pub fn get_mcp_client(engine_state: &EngineState) -> Result<Arc<McpClient>, &'static str> {
        engine_state
            .get_mcp_client()
            .ok_or("MCP client not available")
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
