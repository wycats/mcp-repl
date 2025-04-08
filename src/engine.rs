use async_lock::{Mutex, MutexGuard};
use async_once_cell::OnceCell;
use nu_protocol::engine::EngineState;
use tokio::runtime::Runtime;

use crate::mcp_manager::McpClientManager;

/// Extension trait for `EngineState` to add MCP client, manager, and runtime functionality
pub trait EngineStateExt {
    // New methods for client manager
    async fn get_mcp_client_manager(&self) -> MutexGuard<'static, McpClientManager>;
}

static MCP_CLIENT_MANAGER_STORE: OnceCell<Mutex<McpClientManager>> = OnceCell::new();

pub async fn get_mcp_client_manager() -> MutexGuard<'static, McpClientManager> {
    MCP_CLIENT_MANAGER_STORE
        .get_or_init(async { Mutex::new(McpClientManager::default()) })
        .await
        .lock()
        .await
}

pub fn get_mcp_client_manager_sync() -> MutexGuard<'static, McpClientManager> {
    let rt = Runtime::new().unwrap();
    rt.block_on(get_mcp_client_manager())
}

impl EngineStateExt for EngineState {
    // Get the MCP client manager
    async fn get_mcp_client_manager(&self) -> MutexGuard<'static, McpClientManager> {
        get_mcp_client_manager().await
    }
}
