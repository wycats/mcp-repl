use nu_protocol::engine::EngineState;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::mcp_manager::McpClientManager;

/// Extension trait for EngineState to add MCP client, manager, and runtime functionality
pub trait EngineStateExt {
    // New methods for client manager
    fn get_mcp_client_manager(&self) -> MutexGuard<McpClientManager>;
}

static MCP_CLIENT_MANAGER_STORE: OnceLock<Mutex<McpClientManager>> = OnceLock::new();

pub fn get_mcp_client_manager() -> MutexGuard<'static, McpClientManager> {
    MCP_CLIENT_MANAGER_STORE
        .get_or_init(|| Mutex::new(McpClientManager::new()))
        .lock()
        .unwrap()
}

impl EngineStateExt for EngineState {
    // Get the MCP client manager
    fn get_mcp_client_manager(&self) -> MutexGuard<'static, McpClientManager> {
        get_mcp_client_manager()
    }
}
