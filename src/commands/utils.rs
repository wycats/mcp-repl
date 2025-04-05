use anyhow::{Context, Result};
use nu_protocol::{Record, Span, Value, engine::EngineState};
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::engine::EngineStateExt;
use crate::mcp::McpClient;

/// Set the MCP client in the engine state
pub fn set_mcp_client(engine_state: &mut EngineState, client: Arc<McpClient>) {
    engine_state.set_mcp_client(client);
}

/// Get the MCP client from the engine state
pub fn get_mcp_client(engine_state: &EngineState) -> Result<Arc<McpClient>> {
    engine_state
        .get_mcp_client()
        .ok_or_else(|| anyhow::anyhow!("MCP client not found in engine state"))
}

/// Set the Tokio runtime in the engine state
pub fn set_tokio_runtime(engine_state: &mut EngineState, runtime: Arc<Runtime>) {
    engine_state.set_tokio_runtime(runtime);
}

/// Get the Tokio runtime from the engine state
pub fn get_tokio_runtime(engine_state: &EngineState) -> Result<Arc<Runtime>> {
    engine_state
        .get_tokio_runtime()
        .ok_or_else(|| anyhow::anyhow!("Tokio runtime not found in engine state"))
}

// Helper to convert string map to Record
pub fn string_map_to_record(map: std::collections::HashMap<String, String>, span: Span) -> Record {
    let mut record = Record::new();

    for (k, v) in map {
        record.push(k, Value::string(v, span));
    }

    record
}
