use std::sync::Arc;

use anyhow::Result;
use derive_new::new;
use indexmap::IndexMap;
use log::info;
use nu_protocol::engine::EngineState;
use rmcp::model::Tool;
use todo_by::todo_by;

use crate::commands::utils::ReplClient;

/// Manager for MCP clients to support multiple simultaneous connections
#[derive(Default, new)]
pub struct McpClientManager {
    /// Map of client name to registered tools
    /// This stores the tools registered from each client with their original schemas
    servers: IndexMap<String, RegisteredServer>,
}

#[derive(Debug, Clone)]
pub struct RegisteredServer {
    pub client: Arc<ReplClient>,
    pub tools: IndexMap<String, RegisteredTool>,
}

impl RegisteredServer {
    #[must_use]
    pub const fn new(client: Arc<ReplClient>, tools: IndexMap<String, RegisteredTool>) -> Self {
        Self { client, tools }
    }
}

todo_by!("2025-04-10", "Actually use these fields");

/// A tool that has been registered with the system
#[derive(Clone, Debug)]
pub struct RegisteredTool {
    /// The MCP tool object
    pub tool: Tool,

    /// The namespace of the client,
    #[allow(dead_code)]
    pub namespace: String,
    #[allow(dead_code)]
    pub name: String,

    /// The raw schema JSON from the tool
    #[allow(dead_code)]
    pub raw_schema: nu_protocol::Value,

    /// The client this tool belongs to
    #[allow(dead_code)]
    pub client: Arc<ReplClient>,
}

impl McpClientManager {
    /// Register a new MCP client
    pub fn register_client(
        &mut self,
        name: String,
        client: &Arc<ReplClient>,
        engine_state: &mut EngineState,
    ) -> Result<()> {
        // Store the client by name
        info!("Registering tools from client '{name}'...");
        // engine_state.get_mcp_client_manager()
        let tools = crate::commands::mcp_tools::register_mcp_tools(&name, engine_state, client)?;
        self.servers.insert(name, tools);

        Ok(())
    }

    /// Get all registered clients
    #[must_use]
    pub const fn get_servers(&self) -> &IndexMap<String, RegisteredServer> {
        &self.servers
    }
}
