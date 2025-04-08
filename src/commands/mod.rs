use nu_protocol::engine::{EngineState, StateWorkingSet};

pub mod builtin;
pub mod help;
pub mod list_resources;
pub mod mcp_tools;
pub mod tool;
pub mod tool_mapper;
pub mod utils;

use list_resources::ListResourcesCommand;
use tool::{ToolCommand, ToolListCommand};

// Register all custom commands
pub fn register_all(engine_state: &mut EngineState) {
    // Create a working set to register commands
    let mut working_set = StateWorkingSet::new(engine_state);

    // Register custom MCP commands
    working_set.add_decl(Box::new(ToolCommand {}));
    working_set.add_decl(Box::new(ToolListCommand {}));
    working_set.add_decl(Box::new(ListResourcesCommand {}));

    // Apply the changes
    let delta = working_set.render();
    if let Err(err) = engine_state.merge_delta(delta) {
        log::warn!("Error registering custom commands: {err:?}");
    }
}
