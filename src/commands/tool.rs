use std::sync::Arc;

use anyhow::Result;
use nu_engine::CallExt;
use nu_protocol::{
    Category, IntoPipelineData, PipelineData, ShellError, Signature, Span, Type, Value,
    engine::{Call, Command, EngineState, Stack, StateWorkingSet},
};
use tokio::runtime::Runtime;
// Command for dynamic tool usage
#[derive(Clone)]
pub struct ToolCommand;

impl Command for ToolCommand {
    fn name(&self) -> &'static str {
        "tool"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool")
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Nothing, Type::String)])
    }

    fn description(&self) -> &'static str {
        "Various commands for interacting with MCP tools"
    }

    fn extra_description(&self) -> &'static str {
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

/// Command to list all available dynamic commands
#[derive(Clone)]
pub struct ToolListCommand;

impl Command for ToolListCommand {
    fn name(&self) -> &'static str {
        "tool list"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool list")
            .category(Category::Custom("mcp".into()))
            .switch(
                "protocol",
                "Include protocol information for each tool",
                Some('p'),
            )
            .input_output_types(vec![(Type::Any, Type::Table(vec![].into()))])
    }

    fn description(&self) -> &'static str {
        "List available dynamic commands"
    }

    fn extra_description(&self) -> &'static str {
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
        Ok(list_tool_commands(
            engine_state,
            call,
            call.get_flag_span(stack, "protocol"),
        ))
    }
}

/// Register a dynamic command using the tool system
pub fn register_dynamic_tool(
    working_set: &mut StateWorkingSet,
    name: &str,
    signature: Signature,
    description: String,
    run_fn: Box<RunFn>,
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

pub type RunFn = dyn Fn(&EngineState, &mut Stack, &Call, PipelineData) -> Result<PipelineData, ShellError>
    + Send
    + Sync
    + 'static;

/// A command that wraps a function for dynamic execution
#[derive(Clone)]
struct DynamicToolCommand {
    name: String,
    signature: Signature,
    description: String,
    run_fn: Arc<RunFn>,
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
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        (self.run_fn)(engine_state, stack, call, input)
    }
}

use crate::{engine::EngineStateExt, util::format::json_to_nu};

/// List all commands under the tool namespace
///
/// # Panics
///
/// Panics if the runtime cannot be initialized
pub fn list_tool_commands(
    engine_state: &EngineState,
    call: &Call,
    protocol: Option<Span>,
) -> PipelineData {
    // Get the registered tools from the MCP client manager
    let rt = Runtime::new().unwrap();

    let client_manager = rt.block_on(engine_state.get_mcp_client_manager());
    let servers = client_manager.get_servers();

    let mut values = Vec::new();
    let mut idx = 0;

    // Create a record for each registered tool
    for (client_name, server) in servers {
        for (tool_name, registered_tool) in &server.tools {
            let tool = &registered_tool.tool;
            let mut record = nu_protocol::Record::new();

            record.push("#", Value::int(i64::from(idx), call.head));
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

    drop(client_manager);

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

    Value::list(values, call.head).into_pipeline_data()
}
