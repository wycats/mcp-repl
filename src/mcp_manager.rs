use anyhow::{Result, anyhow};
use log::info;
use nu_protocol::engine::EngineState;
use std::{collections::HashMap, sync::Arc};

use crate::commands::utils::ReplClient;

/// Manager for MCP clients to support multiple simultaneous connections
pub struct McpClientManager {
    /// Map of client name to client
    clients: HashMap<String, Arc<ReplClient>>,
}

impl McpClientManager {
    /// Create a new MCP client manager
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
}

impl McpClientManager {
    /// Register a new MCP client
    pub fn register_client(
        &mut self,
        name: String,
        client: Arc<ReplClient>,
        engine_state: &mut EngineState,
    ) -> Result<()> {
        let client_name = client.name.clone();
        // Store the client by name
        self.clients.insert(name, client.clone());
        info!("Registering tools from client '{}'...", client_name);
        // engine_state.get_mcp_client_manager()
        crate::commands::mcp_tools::register_mcp_tools(engine_state, &client)?;

        Ok(())
    }

    /// Clear clients
    pub fn clear_clients(&mut self) -> anyhow::Result<()> {
        self.clients.clear();
        Ok(())
    }

    /// Get a client by name, or the first available client if name is None
    pub fn get_client(&self, name: Option<&str>) -> anyhow::Result<Arc<ReplClient>> {
        match name {
            Some(name) => {
                if let Some(client) = self.clients.get(name) {
                    Ok(client.clone())
                } else {
                    Err(anyhow::anyhow!("MCP client '{}' not found", name))
                }
            }
            None => {
                // If no name provided, return the first client if available
                if let Some((_name, client)) = self.clients.iter().next() {
                    Ok(client.clone())
                } else {
                    Err(anyhow::anyhow!("No MCP clients available"))
                }
            }
        }
    }

    /// Get all clients
    pub fn get_clients(&self) -> HashMap<String, Arc<ReplClient>> {
        self.clients.clone()
    }

    /// Check if a client with the given name exists
    pub fn has_client(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    /// Remove a client by name
    pub fn remove_client(&mut self, name: &str) -> Result<()> {
        if !self.has_client(name) {
            return Err(anyhow!("No MCP client found with name: {}", name));
        }

        // Remove the client from the map
        self.clients.remove(name);

        // No need to update active client reference since we've removed that concept

        Ok(())
    }
}
