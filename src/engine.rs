use crate::mcp::McpClient;
use nu_protocol::engine::EngineState;
use std::sync::{Arc, Mutex};

/// Extension trait for EngineState to add MCP client functionality
pub trait EngineStateExt {
    fn get_mcp_client(&self) -> Option<Arc<McpClient>>;
    fn set_mcp_client(&mut self, client: Arc<McpClient>);
}

/// Store for MCP client state
pub struct McpClientStore {
    client: Arc<Mutex<Option<Arc<McpClient>>>>,
}

impl McpClientStore {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }
}

// Store the MCP client in a thread-local to avoid modifying EngineState
thread_local! {
    static MCP_CLIENT_STORE: McpClientStore = McpClientStore::new();
}

impl EngineStateExt for EngineState {
    fn get_mcp_client(&self) -> Option<Arc<McpClient>> {
        MCP_CLIENT_STORE.with(|store| store.client.lock().ok().and_then(|guard| guard.clone()))
    }

    fn set_mcp_client(&mut self, client: Arc<McpClient>) {
        MCP_CLIENT_STORE.with(|store| {
            if let Ok(mut guard) = store.client.lock() {
                *guard = Some(client);
            }
        });
    }
}
