use anyhow::Result;
use nu_protocol::engine::EngineState;
use nu_protocol::engine::StateWorkingSet;

pub mod builtin;
pub mod call_tool;
pub mod dynamic_commands;
pub mod help;
pub mod list_tools;
pub mod mcp_tools;
pub mod test_commands;
pub mod tool;
pub mod tool_mapper;
pub mod utils;

use call_tool::CallToolCommand;
use list_tools::ListToolsCommand;
use tool::{ToolCommand, ToolListCommand};

// Register all custom commands
pub fn register_all(engine_state: &mut EngineState) {
    // Create a working set to register commands
    let mut working_set = StateWorkingSet::new(engine_state);

    // Register custom MCP commands
    working_set.add_decl(Box::new(ListToolsCommand {}));
    working_set.add_decl(Box::new(CallToolCommand {}));
    working_set.add_decl(Box::new(ToolCommand {}));
    working_set.add_decl(Box::new(ToolListCommand {}));

    // Apply the changes
    let delta = working_set.render();
    if let Err(err) = engine_state.merge_delta(delta) {
        log::warn!("Error registering custom commands: {:?}", err);
    }

    // Register MCP tools as dynamic commands
    // This happens after engine state is initialized so we can access the MCP client
    log::info!("Attempting to register MCP tools as dynamic commands");

    // Check if MCP client is available yet
    match utils::get_mcp_client(engine_state) {
        Ok(client) => {
            let tools_count = client.get_tools().len();
            log::info!("Found MCP client with {} tools", tools_count);

            if let Err(err) = mcp_tools::register_mcp_tools(engine_state) {
                log::warn!("Failed to register MCP tools as dynamic commands: {}", err);
            } else {
                log::info!("Successfully registered MCP tools as dynamic commands");
            }
        }
        Err(err) => {
            log::warn!(
                "Could not register MCP tools yet - MCP client not available: {}",
                err
            );
            log::warn!("Tools will be registered later when MCP client is initialized");
        }
    }
}

// Register test dynamic commands for development purposes
pub fn register_test_commands(engine_state: &mut EngineState) -> Result<()> {
    // Use our new implementation from test_commands.rs
    test_commands::register_test_commands(engine_state)?;

    // Log registered commands
    if let Ok(registry) = utils::get_command_registry() {
        if let Ok(registry_guard) = registry.lock() {
            let commands = registry_guard.get_command_names();
            log::info!("Registered dynamic commands: {:?}", commands);
        }
    }

    Ok(())
}
