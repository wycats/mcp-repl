use anyhow::{Result, anyhow};
use derive_new::new;
use indexmap::IndexMap;
use log::info;
use nu_protocol::engine::EngineState;
use rmcp::model::Tool;
use serde_json::Value as JsonValue;
use std::{collections::HashMap, sync::Arc};

use crate::commands::utils::ReplClient;

/// Manager for MCP clients to support multiple simultaneous connections
#[derive(Default, new)]
pub struct McpClientManager {
    /// Map of client name to registered tools
    /// This stores the tools registered from each client with their original schemas
    registered_tools: IndexMap<String, RegisteredServer>,
}

#[derive(Debug, Clone)]
pub struct RegisteredServer {
    pub client: Arc<ReplClient>,
    pub tools: IndexMap<String, RegisteredTool>,
}

impl RegisteredServer {
    pub fn new(client: Arc<ReplClient>, tools: IndexMap<String, RegisteredTool>) -> Self {
        Self { client, tools }
    }

    pub fn build(client: Arc<ReplClient>) -> Self {
        Self {
            client,
            tools: IndexMap::new(),
        }
    }
}

/// A tool that has been registered with the system
#[derive(Clone, Debug)]
pub struct RegisteredTool {
    /// The MCP tool object
    pub tool: Tool,

    /// The namespace of the client,
    pub namespace: String,
    pub name: String,

    /// The raw schema JSON from the tool
    pub raw_schema: nu_protocol::Value,

    /// The client this tool belongs to
    pub client: Arc<ReplClient>,
}

impl McpClientManager {
    /// Register a new MCP client
    pub fn register_client(
        &mut self,
        name: String,
        client: Arc<ReplClient>,
        engine_state: &mut EngineState,
    ) -> Result<()> {
        // Store the client by name
        info!("Registering tools from client '{}'...", name);
        // engine_state.get_mcp_client_manager()
        let tools = crate::commands::mcp_tools::register_mcp_tools(&name, engine_state, &client)?;
        self.registered_tools.insert(name, tools);

        Ok(())
    }

    /// Clear clients
    pub fn clear_clients(&mut self) -> anyhow::Result<()> {
        self.registered_tools.clear();
        Ok(())
    }

    pub fn get_server(&self, name: &str) -> anyhow::Result<RegisteredServer> {
        self.registered_tools
            .get(name)
            .cloned()
            .ok_or(anyhow::anyhow!("MCP client '{}' not found", name))
    }

    pub fn has_server(&self, name: &str) -> bool {
        self.registered_tools.contains_key(name)
    }

    /// Get all registered clients
    pub fn get_servers(&self) -> &IndexMap<String, RegisteredServer> {
        &self.registered_tools
    }
}
