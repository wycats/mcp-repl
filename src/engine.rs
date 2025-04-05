use crate::mcp::McpClient;
use nu_protocol::engine::EngineState;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

/// Extension trait for EngineState to add MCP client and runtime functionality
pub trait EngineStateExt {
    fn get_mcp_client(&self) -> Option<Arc<McpClient>>;
    fn set_mcp_client(&mut self, client: Arc<McpClient>);
    fn get_tokio_runtime(&self) -> Option<Arc<Runtime>>;
    fn set_tokio_runtime(&mut self, runtime: Arc<Runtime>);
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

/// Store for Tokio runtime state
pub struct RuntimeStore {
    runtime: Arc<Mutex<Option<Arc<Runtime>>>>,
}

impl RuntimeStore {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Mutex::new(None)),
        }
    }
}

// Store the MCP client and runtime in thread-locals to avoid modifying EngineState
thread_local! {
    static MCP_CLIENT_STORE: McpClientStore = McpClientStore::new();
    static RUNTIME_STORE: RuntimeStore = RuntimeStore::new();
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
    
    fn get_tokio_runtime(&self) -> Option<Arc<Runtime>> {
        RUNTIME_STORE.with(|store| store.runtime.lock().ok().and_then(|guard| guard.clone()))
    }

    fn set_tokio_runtime(&mut self, runtime: Arc<Runtime>) {
        RUNTIME_STORE.with(|store| {
            if let Ok(mut guard) = store.runtime.lock() {
                *guard = Some(runtime);
            }
        });
    }
}
