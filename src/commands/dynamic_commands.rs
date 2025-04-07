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
    use anyhow::Result;
    use nu_protocol::{
        IntoPipelineData, PipelineData, ShellError, Span, Value,
        engine::{Call, EngineState, Stack},
    };

    use crate::{commands::utils::get_command_registry, util::format::json_to_nu_result};
    use crate::{engine::EngineStateExt, util::format::json_to_nu};

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
        protocol: Option<Span>,
    ) -> Result<PipelineData, ShellError> {
        // Get the registered tools from the MCP client manager
        let client_manager = engine_state.get_mcp_client_manager();
        let servers = client_manager.get_servers();

        let mut values = Vec::new();
        let mut idx = 0;

        // Create a record for each registered tool
        for (client_name, server) in servers.iter() {
            for (tool_name, registered_tool) in server.tools.iter() {
                let tool = &registered_tool.tool;
                let mut record = nu_protocol::Record::new();

                record.push("#", Value::int(idx as i64, call.head));
                idx += 1;

                // Add the client name for filtering/grouping
                record.push("client", Value::string(client_name.clone(), call.head));

                // The fully qualified tool name (client.tool format)
                record.push("name", Value::string(tool_name, call.head));

                // Add description if available
                if let Some(desc) = &tool.description {
                    record.push("description", Value::string(desc.clone(), call.head));
                } else {
                    record.push("description", Value::string("", call.head));
                }

                if let Some(protocol) = protocol {
                    record.push(
                        "protocol",
                        json_to_nu(&tool.schema_as_json_value(), Some(protocol)),
                    );
                }

                values.push(Value::record(record, call.head));
            }
        }

        if values.is_empty() {
            // Add a message when no tools are found
            let mut record = nu_protocol::Record::new();
            record.push(
                "message",
                Value::string(
                    "No registered MCP tools found. Try connecting to an MCP server first.",
                    call.head,
                ),
            );
            values.push(Value::record(record, call.head));
        }

        Ok(Value::list(values, call.head).into_pipeline_data())
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
