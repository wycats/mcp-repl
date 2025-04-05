use anyhow::Result;
use nu_engine::CallExt;
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, SyntaxShape, Type, Value,
    engine::{Call, Command, EngineState, Stack},
};

use crate::commands::utils;

/// A simple test command to demonstrate dynamic command registration
#[derive(Clone)]
pub struct TestDynamicCommand {
    name: String,
    description: String,
}

impl TestDynamicCommand {
    pub fn new(name: String, description: String) -> Self {
        Self { name, description }
    }
}

impl Command for TestDynamicCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn signature(&self) -> Signature {
        Signature::build(&self.name)
            .optional(
                "message",
                SyntaxShape::String,
                "An optional message to echo back",
            )
            .category(Category::Custom("dynamic".into()))
            .input_output_types(vec![(Type::Any, Type::String)])
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let message: Option<String> = call.opt(engine_state, stack, 0)?;
        let message = message.unwrap_or_else(|| "Hello from a dynamic command!".to_string());

        let output = format!("Dynamic command '{}' says: {}", self.name, message);
        Ok(PipelineData::Value(Value::string(output, call.head), None))
    }
}

pub mod dynamic_tool_commands {
    use nu_protocol::IntoPipelineData;

    use crate::commands::utils::get_command_registry;

    use super::*;

    /// Execute a dynamic command with the given name, supporting namespaced commands
    pub fn execute_dynamic_command(
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        cmd_name: &str,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // Look up the command in the registry
        match get_command_registry() {
            Ok(registry) => {
                if let Ok(registry_guard) = registry.lock() {
                    // For namespaced commands (like 'tool hello'), we need to look up the full name
                    if registry_guard.is_registered(cmd_name) {
                        if let Some(cmd_info) = registry_guard.get_command_info(cmd_name) {
                            // Get the declaration by ID
                            let decl = engine_state.get_decl(cmd_info.decl_id);

                            // Execute the command with the inputs
                            return Ok(decl.run(engine_state, stack, call, input)?);
                        }
                    }

                    // If command is entered as a subcommand like 'tool hello',
                    // try looking up the full namespaced name 'tool hello'
                    if !cmd_name.starts_with("tool ") {
                        let namespaced_name = format!("tool {}", cmd_name);
                        if registry_guard.is_registered(&namespaced_name) {
                            if let Some(cmd_info) =
                                registry_guard.get_command_info(&namespaced_name)
                            {
                                // Get the declaration by ID
                                let decl = engine_state.get_decl(cmd_info.decl_id);

                                // Execute the command with the inputs
                                return Ok(decl.run(engine_state, stack, call, input)?);
                            }
                        }
                    }

                    // Command not found in registry
                    Err(ShellError::GenericError {
                        error: "Command not found".into(),
                        msg: format!("The command '{}' is not registered", cmd_name),
                        span: Some(call.head),
                        help: Some("Run 'tool list' to see available commands".into()),
                        inner: vec![],
                    })
                } else {
                    // Couldn't lock registry
                    Err(ShellError::GenericError {
                        error: "Registry error".into(),
                        msg: "Could not access the command registry".into(),
                        span: Some(call.head),
                        help: None,
                        inner: vec![],
                    })
                }
            }
            Err(e) => {
                // Couldn't get registry
                Err(ShellError::GenericError {
                    error: "Registry error".into(),
                    msg: e.to_string(),
                    span: Some(call.head),
                    help: None,
                    inner: vec![],
                })
            }
        }
    }

    /// List all commands under the tool namespace
    pub fn list_tool_commands(
        engine_state: &EngineState,
        call: &Call,
    ) -> Result<PipelineData, ShellError> {
        match get_command_registry() {
            Ok(registry) => {
                if let Ok(registry_guard) = registry.lock() {
                    let commands = registry_guard.get_command_names();
                    let mut values = Vec::new();

                    // Create record for each tool command (that starts with "tool ")
                    for (idx, name) in commands.iter().enumerate() {
                        if name.starts_with("tool ") {
                            if let Some(cmd_info) = registry_guard.get_command_info(name) {
                                // Get the declaration by ID
                                let decl = engine_state.get_decl(cmd_info.decl_id);
                                let mut record = nu_protocol::Record::new();
                                record.push("#", Value::int(idx as i64, call.head));

                                // Display the subcommand part (without the "tool " prefix)
                                let subcommand = name.strip_prefix("tool ").unwrap_or(name);
                                record.push("name", Value::string(subcommand, call.head));
                                record.push(
                                    "description",
                                    Value::string(decl.description(), call.head),
                                );
                                record.push("category", Value::string("tool", call.head));

                                values.push(Value::record(record, call.head));
                            }
                        }
                    }

                    Ok(Value::list(values, call.head).into_pipeline_data())
                } else {
                    Err(ShellError::GenericError {
                        error: "Registry lock failed".into(),
                        msg: "Could not access the dynamic command registry".into(),
                        span: Some(call.head),
                        help: None,
                        inner: vec![],
                    })
                }
            }
            Err(e) => Err(ShellError::GenericError {
                error: "Registry error".into(),
                msg: e.to_string(),
                span: Some(call.head),
                help: None,
                inner: vec![],
            }),
        }
    }
}

/// Register a test dynamic command for development
pub fn register_test_command(engine_state: &mut EngineState, name: &str) -> Result<()> {
    let command = TestDynamicCommand::new(
        name.to_string(),
        format!("A test dynamic command named '{}'", name),
    );

    // Register the command using our dynamic command registry
    utils::register_dynamic_command(engine_state, Box::new(command))?;

    Ok(())
}
