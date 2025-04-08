use std::{borrow::Cow, sync::Arc};

use anyhow::Result;
use indexmap::IndexMap;
use log::info;
use nu_protocol::{PipelineData, ShellError, Span, Value, engine::EngineState};
use rmcp::model::Tool;
use serde_json::Value as JsonValue;
use tokio::runtime::Runtime;

use super::{tool::RunFn, tool_mapper, utils::ReplClient};
use crate::{
    commands::tool::register_dynamic_tool,
    mcp_manager::{RegisteredServer, RegisteredTool},
    util::format::json_to_nu,
};

/// Register all MCP tools as Nushell commands using `StateWorkingSet` directly
/// This allows us to register tools even from within a command that only has
/// an immutable reference to `EngineState`
pub fn register_mcp_tools_in_working_set(
    name: &str,
    working_set: &mut nu_protocol::engine::StateWorkingSet,
    client: &Arc<ReplClient>,
) -> IndexMap<String, RegisteredTool> {
    let tools = client.get_tools();
    let mut registered_tools = IndexMap::new();

    info!(
        "Registering {} MCP tools from client '{}' (raw name: {}) under namespace 'tool'",
        tools.len(),
        client.name,
        name
    );

    for tool in tools {
        // Extract the raw schema JSON before registration
        let schema = tool.input_schema.as_ref();
        let raw_schema = serde_json::to_value(schema).unwrap_or(JsonValue::Null);

        // Register the tool as a command
        register_mcp_tool_in_working_set(name, working_set, tool, client);
        registered_tools.insert(
            tool.name.to_string(),
            RegisteredTool {
                tool: tool.clone(),
                namespace: client.name.clone(),
                name: tool.name.to_string(),
                raw_schema: json_to_nu(&raw_schema, Some(Span::unknown())),
                client: client.clone(),
            },
        );
    }

    registered_tools
}

/// Register all MCP tools as Nushell commands using the standard approach with mutable `EngineState`
pub fn register_mcp_tools(
    name: &str,
    engine_state: &mut EngineState,
    client: &Arc<ReplClient>,
) -> Result<RegisteredServer> {
    let tools = client.get_tools();

    info!(
        "Registering {} MCP tools from client '{}' (raw name: {}) under namespace 'tool'",
        tools.len(),
        name,
        client.name
    );

    // Use StateWorkingSet internally for consistency
    let mut working_set = nu_protocol::engine::StateWorkingSet::new(engine_state);

    let registered_tools = register_mcp_tools_in_working_set(name, &mut working_set, client);

    // Apply the changes to the engine state
    let delta = working_set.render();
    engine_state.merge_delta(delta)?;

    Ok(RegisteredServer::new(client.clone(), registered_tools))
}

/// Register a single MCP tool as a Nushell command using `StateWorkingSet`
/// This version works with an immutable `EngineState` reference by using `StateWorkingSet`
fn register_mcp_tool_in_working_set(
    mcp_namespace: &str,
    working_set: &mut nu_protocol::engine::StateWorkingSet,
    tool: &Tool,
    client: &Arc<ReplClient>,
) {
    // Get tool information
    let tool_name = tool.name.clone();
    let tool_description = tool.description.clone();

    // Create the namespaced C name
    // Format: "tool mcp_namespace.tool_name"
    let namespaced_tool_name = format!("{mcp_namespace}.{tool_name}");
    let command_name = format!("tool {namespaced_tool_name}");

    // Generate the command signature
    let signature = tool_mapper::map_tool_to_signature(tool, "tool");

    info!("Registering MCP tool as command: {command_name}");

    // Generate a help description from the tool
    let description = tool_description;

    // Create a run function that will call the tool when the command is invoked
    let run_fn = create_tool_run_function(tool.clone(), client);

    // Create a dynamic command using a custom implementation
    // that follows the same pattern as super::tool::register_dynamic_tool
    // but works with StateWorkingSet

    let desc_clone = description.clone().unwrap_or(Cow::Borrowed(""));

    // We need to create a Command implementation
    register_dynamic_tool(
        working_set,
        &command_name,
        signature,
        desc_clone.to_string(),
        run_fn,
    );
}

/// Create a run function for the MCP tool
fn create_tool_run_function(tool: Tool, client: &Arc<ReplClient>) -> Box<RunFn> {
    let client = client.clone();
    Box::new(move |engine_state, stack, call, _input| {
        let span = call.head;
        let tool_name = tool.name.to_string();

        // Map call arguments to tool parameters
        let params =
            match tool_mapper::map_call_args_to_tool_params(engine_state, stack, call, &tool) {
                Ok(params) => params,
                Err(err) => {
                    return Err(ShellError::GenericError {
                        error: "Failed to parse tool parameters".into(),
                        msg: err.to_string(),
                        span: Some(span),
                        help: Some(
                            "Check that the provided arguments match the tool's requirements"
                                .into(),
                        ),
                        inner: Vec::new(),
                    });
                }
            };

        // Create the arguments JSON value
        let args_json = serde_json::json!(params);

        // We need to avoid calling block_on within a Tokio runtime, which causes panic
        // Use a separate thread with its own runtime to execute the async call
        let client_clone = client.clone();
        let tool_name_clone = tool_name.clone();

        // Create a channel to receive the result
        let (sender, receiver) = std::sync::mpsc::channel();

        // Spawn a new thread that will handle the async work
        std::thread::spawn(move || {
            // Create a new runtime in this separate thread
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = sender.send(Err(anyhow::anyhow!("Failed to create runtime: {}", e)));
                    return;
                }
            };

            // Execute the async call in the new runtime
            let result = rt.block_on(async {
                // Pass the debug flag from the ReplClient
                client_clone.call_tool(&tool_name_clone, args_json).await
            });

            // Send the result back through the channel
            let _ = sender.send(result);
        });

        // Receive the result from the channel
        let result = match receiver.recv() {
            Ok(result) => result,
            Err(err) => {
                return Err(ShellError::GenericError {
                    error: "Failed to call MCP tool".into(),
                    msg: format!("Channel error: {err}"),
                    span: Some(span),
                    help: Some(format!("Error calling tool: {tool_name}")),
                    inner: Vec::new(),
                });
            }
        };

        // Process the result
        match result {
            Ok(contents) => {
                // Convert the result to Nushell values
                let mut values = Vec::new();

                for content in contents {
                    // Extract the raw content from the annotated wrapper
                    let raw_content = &content.raw;

                    match raw_content {
                        rmcp::model::RawContent::Text(text_content) => {
                            values.push(Value::string(&text_content.text, span));
                        }
                        rmcp::model::RawContent::Image(image_content) => {
                            values.push(Value::string(
                                format!(
                                    "[Image: {} bytes, type: {}]",
                                    image_content.data.len(),
                                    image_content.mime_type
                                ),
                                span,
                            ));
                        }
                        rmcp::model::RawContent::Resource(resource) => {
                            // Handle embedded resources
                            match &resource.resource {
                                rmcp::model::ResourceContents::TextResourceContents {
                                    text,
                                    ..
                                } => {
                                    values.push(Value::string(text, span));
                                }
                                rmcp::model::ResourceContents::BlobResourceContents { .. } => {
                                    values
                                        .push(Value::string("[Resource: Non-text resource]", span));
                                }
                            }
                        }
                    }
                }

                // Return appropriate data based on number of values
                if values.is_empty() {
                    Ok(PipelineData::Value(Value::nothing(span), None))
                } else if values.len() == 1 {
                    Ok(PipelineData::Value(values[0].clone(), None))
                } else {
                    Ok(PipelineData::Value(Value::list(values, span), None))
                }
            }
            Err(err) => Err(ShellError::GenericError {
                error: "Tool execution failed".into(),
                msg: err.to_string(),
                span: Some(span),
                help: Some("Check tool parameters and try again".into()),
                inner: Vec::new(),
            }),
        }
    })
}
