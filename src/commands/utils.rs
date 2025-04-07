use crate::mcp::McpClient;
use anyhow::Result;
use nu_protocol::DeclId;
use nu_protocol::{
    Record, Span, Value,
    engine::{Command, EngineState, StateWorkingSet},
};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ReplClient {
    pub(crate) name: String,
    pub(crate) client: McpClient,
    pub(crate) debug: bool,
}

impl Deref for ReplClient {
    type Target = McpClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

// Helper to convert string map to Record
pub fn string_map_to_record(map: std::collections::HashMap<String, String>, span: Span) -> Record {
    let mut record = Record::new();

    for (k, v) in map {
        record.push(k, Value::string(v, span));
    }

    record
}

/// Structure to track registered dynamic commands
pub struct DynamicCommandRegistry {
    // Map of command name to command info
    command_info: HashMap<String, DynamicCommandInfo>,
}

/// Information about a registered dynamic command
pub struct DynamicCommandInfo {
    pub name: String,
    pub decl_id: DeclId, // Store declaration ID from engine state
}

impl DynamicCommandRegistry {
    pub fn new() -> Self {
        Self {
            command_info: HashMap::new(),
        }
    }

    /// Register a command was registered by name and ID
    pub fn register(&mut self, name: String, decl_id: DeclId) {
        self.command_info
            .insert(name.clone(), DynamicCommandInfo { name, decl_id });
    }

    /// Check if a command is already registered
    pub fn is_registered(&self, name: &str) -> bool {
        self.command_info.contains_key(name)
    }

    /// Get information about a registered command
    pub fn get_command_info(&self, name: &str) -> Option<&DynamicCommandInfo> {
        self.command_info.get(name)
    }

    /// Get all registered command names
    pub fn get_command_names(&self) -> Vec<String> {
        self.command_info.keys().cloned().collect()
    }
}

/// Thread-local storage for dynamic command registry
pub struct CommandRegistryStore {
    registry: Arc<Mutex<DynamicCommandRegistry>>,
}

impl CommandRegistryStore {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(DynamicCommandRegistry::new())),
        }
    }
}

// Thread-local storage for command registry
thread_local! {
    static CMD_REGISTRY_STORE: CommandRegistryStore = CommandRegistryStore::new();
}

/// Get the dynamic command registry
pub fn get_command_registry() -> Result<Arc<Mutex<DynamicCommandRegistry>>> {
    let result = CMD_REGISTRY_STORE.with(|store| store.registry.clone());
    Ok(result)
}

/// Register a dynamic command with Nushell
pub fn register_dynamic_command(
    engine_state: &mut EngineState,
    command: Box<dyn Command>,
) -> Result<()> {
    let command_name = command.name().to_string();

    // Create a working set to register the command
    let mut working_set = StateWorkingSet::new(engine_state);

    // Add the declaration to the working set
    let decl_id = working_set.add_decl(command);

    // Apply the changes to the engine state
    let delta = working_set.render();

    engine_state.merge_delta(delta)?;

    // Store the command info in our registry
    CMD_REGISTRY_STORE.with(|store| {
        if let Ok(mut registry) = store.registry.lock() {
            registry.register(command_name, decl_id);
        }
    });

    Ok(())
}
