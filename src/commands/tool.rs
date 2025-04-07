use anyhow::Result;
use nu_protocol::{
    Category, IntoPipelineData, PipelineData, ShellError, Signature, Type, Value,
    engine::{Call, Command, EngineState, Stack, StateWorkingSet},
};
use std::sync::Arc;

use crate::commands::utils::{self};

/// Command for dynamic tool usage
#[derive(Clone)]
pub struct ToolCommand;

impl Command for ToolCommand {
    fn name(&self) -> &str {
        "tool"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool")
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Nothing, Type::String)])
    }

    fn description(&self) -> &str {
        "Various commands for interacting with MCP tools"
    }

    fn extra_description(&self) -> &str {
        "You must use one of the following subcommands. Using this command as-is will only produce this help message."
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // Show help when the tool command is called directly without subcommands
        // This mimics the behavior of Nushell's built-in namespaces like 'str'
        Ok(Value::string(
            nu_engine::get_full_help(self, engine_state, stack),
            call.head,
        )
        .into_pipeline_data())
    }
}

// Implementation of ToolCommand methods
impl ToolCommand {
    // Helper method to list all dynamic commands
    fn list_dynamic_commands(
        &self,
        engine_state: &EngineState,
        _stack: &mut Stack,
        call: &Call,
    ) -> Result<PipelineData, ShellError> {
        match utils::get_command_registry() {
            Ok(registry) => {
                if let Ok(registry_guard) = registry.lock() {
                    let commands = registry_guard.get_command_names();
                    let mut values = Vec::new();

                    for (idx, name) in commands.iter().enumerate() {
                        if let Some(cmd_info) = registry_guard.get_command_info(name) {
                            // Get the declaration by ID
                            let decl = engine_state.get_decl(cmd_info.decl_id);
                            let mut record = nu_protocol::Record::new();
                            record.push("#", Value::int(idx as i64, call.head));
                            record.push("name", Value::string(name, call.head));
                            record
                                .push("description", Value::string(decl.description(), call.head));
                            record.push("category", Value::string("dynamic", call.head));

                            values.push(Value::record(record, call.head));
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

/// Command to list all available dynamic commands
#[derive(Clone)]
pub struct ToolListCommand;

impl Command for ToolListCommand {
    fn name(&self) -> &str {
        "tool list"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool list")
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Any, Type::Table(vec![].into()))])
    }

    fn description(&self) -> &str {
        "List available dynamic commands"
    }

    fn extra_description(&self) -> &str {
        "Display a list of all registered dynamic commands"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // Use our new implementation that lists only tool namespace commands
        crate::commands::dynamic_commands::dynamic_tool_commands::list_tool_commands(
            engine_state,
            call,
        )
    }
}

/// Register a dynamic command using the tool system
pub fn register_dynamic_tool(
    working_set: &mut StateWorkingSet,
    name: &str,
    signature: Signature,
    description: String,
    run_fn: Box<
        dyn Fn(&EngineState, &mut Stack, &Call, PipelineData) -> Result<PipelineData, ShellError>
            + Send
            + Sync
            + 'static,
    >,
) {
    // Create a dynamic command that wraps the function
    let command = DynamicToolCommand {
        name: name.to_string(),
        signature,
        description,
        run_fn: Arc::from(run_fn),
    };

    // Register the command
    working_set.add_decl(Box::new(command));
}

/// A command that wraps a function for dynamic execution
#[derive(Clone)]
struct DynamicToolCommand {
    name: String,
    signature: Signature,
    description: String,
    run_fn: Arc<
        dyn Fn(&EngineState, &mut Stack, &Call, PipelineData) -> Result<PipelineData, ShellError>
            + Send
            + Sync
            + 'static,
    >,
}

impl Command for DynamicToolCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn signature(&self) -> Signature {
        self.signature.clone()
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
        (self.run_fn)(engine_state, stack, call, _input)
    }
}
