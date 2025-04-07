use std::borrow::Cow;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::info;
use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::{PipelineData, ShellError, Value};
use rmcp::model::Tool;
use tokio::runtime::Runtime;

use crate::commands::tool::register_dynamic_tool;

use super::tool_mapper;
use super::utils::ReplClient;

/// Register all MCP tools as Nushell commands using StateWorkingSet directly
/// This allows us to register tools even from within a command that only has
/// an immutable reference to EngineState
pub fn register_mcp_tools_in_working_set(
    working_set: &mut nu_protocol::engine::StateWorkingSet,
    client: &Arc<ReplClient>,
) -> Result<()> {
    let tools = client.get_tools();

    info!(
        "Registering {} MCP tools from client '{}' under namespace 'tool'",
        tools.len(),
        client.name
    );

    for tool in tools {
        register_mcp_tool_in_working_set(working_set, &tool, client, &client.name)?;
    }

    Ok(())
}

/// Register all MCP tools as Nushell commands using the standard approach with mutable EngineState
pub fn register_mcp_tools(engine_state: &mut EngineState, client: &Arc<ReplClient>) -> Result<()> {
    let tools = client.get_tools();

    info!(
        "Registering {} MCP tools from client '{}' under namespace 'tool'",
        tools.len(),
        client.name
    );

    // Use StateWorkingSet internally for consistency
    let mut working_set = nu_protocol::engine::StateWorkingSet::new(engine_state);

    for tool in tools {
        register_mcp_tool_in_working_set(&mut working_set, &tool, client, &client.name)?;
    }

    // Apply the changes to the engine state
    let delta = working_set.render();
    engine_state.merge_delta(delta)?;

    Ok(())
}

/// Register a single MCP tool as a Nushell command using StateWorkingSet
/// This version works with an immutable EngineState reference by using StateWorkingSet
fn register_mcp_tool_in_working_set(
    working_set: &mut nu_protocol::engine::StateWorkingSet,
    tool: &Tool,
    client: &Arc<ReplClient>,
    mcp_namespace: &str,
) -> Result<()> {
    // Get tool information
    let tool_name = tool.name.clone();
    let tool_description = tool.description.clone();

    // Create the namespaced C name
    // Format: "tool mcp_namespace.tool_name"
    let namespaced_tool_name = format!("{}.{}", mcp_namespace, tool_name);
    let command_name = format!("tool {}", namespaced_tool_name);

    // Generate the command signature
    let signature = tool_mapper::map_tool_to_signature(tool, "tool")
        .context("Failed to map tool to signature")?;

    info!("Registering MCP tool as command: {}", command_name);

    // Generate a help description from the tool
    let description = tool_description;

    // Create a run function that will call the tool when the command is invoked
    let run_fn = create_tool_run_function(tool.clone(), client);

    // Create a dynamic command using a custom implementation
    // that follows the same pattern as super::tool::register_dynamic_tool
    // but works with StateWorkingSet

    let desc_clone = description.clone().unwrap_or_else(|| Cow::Borrowed(""));

    // We need to create a Command implementation
    register_dynamic_tool(
        working_set,
        &command_name,
        signature,
        desc_clone.to_string(),
        run_fn,
    );

    Ok(())
}

/// Create a run function for the MCP tool
fn create_tool_run_function(
    tool: Tool,
    client: &Arc<ReplClient>,
) -> Box<
    dyn Fn(
            &EngineState,
            &mut Stack,
            &nu_protocol::engine::Call<'_>,
            PipelineData,
        ) -> Result<PipelineData, ShellError>
        + Send
        + Sync
        + 'static,
> {
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
                    msg: format!("Channel error: {}", err),
                    span: Some(span),
                    help: Some(format!("Error calling tool: {}", tool_name)),
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
                                _ => {
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
