use anyhow::{Context, Result};
use log::info;
use nu_protocol::{
    PipelineData, ShellError, Value,
    engine::{EngineState, Stack},
};
use rmcp::model::Tool;
use serde_json::Value as JsonValue;
use tokio::runtime::Runtime;

use super::tool::register_dynamic_tool;
use super::tool_mapper;

/// Register all MCP tools as Nushell commands
pub fn register_mcp_tools(engine_state: &mut EngineState) -> Result<()> {
    // Get the MCP client from engine state
    let client = super::utils::get_mcp_client(engine_state)?;

    // Determine the MCP identifier/namespace
    // For now, we'll use a hardcoded "fs" for the filesystem MCP
    // In the future, this should be determined dynamically from the MCP connection
    let mcp_namespace = client.name.as_str(); // Default namespace for now

    let tools = client.get_tools();
    info!(
        "Registering {} MCP tools under namespace '{}'",
        tools.len(),
        mcp_namespace
    );

    for tool in tools {
        register_mcp_tool(engine_state, &tool, mcp_namespace)?;
    }

    Ok(())
}

/// Register a single MCP tool as a Nushell command
fn register_mcp_tool(
    engine_state: &mut EngineState,
    tool: &Tool,
    mcp_namespace: &str,
) -> Result<()> {
    let tool_name = tool.name.to_string();
    info!("Registering MCP tool: {}.{}", mcp_namespace, tool_name);

    // Map the MCP tool to a Nushell signature
    // Use "tool" as the category for all MCP tools
    let signature = tool_mapper::map_tool_to_signature(tool, "tool")
        .context(format!("Failed to map tool '{}' to signature", tool_name))?;

    // Generate a help description from the tool
    let description = tool_mapper::generate_help_description(tool);

    // Create a run function that will call the tool when the command is invoked
    let run_fn = create_tool_run_function(tool.clone());

    // Create the namespaced command name
    // Format: "tool mcp_namespace.tool_name"
    let namespaced_tool_name = format!("{}.{}", mcp_namespace, tool_name);
    let command_name = format!("tool {}", namespaced_tool_name);

    // Register the tool as a dynamic command with the namespaced identifier
    register_dynamic_tool(engine_state, &command_name, signature, description, run_fn).context(
        format!("Failed to register dynamic tool command: {}", command_name),
    )?;

    Ok(())
}

/// Create a run function for the MCP tool
fn create_tool_run_function(
    tool: Tool,
) -> Box<
    dyn Fn(
            &EngineState,
            &mut Stack,
            &nu_protocol::engine::Call<'_>,
            PipelineData,
        ) -> Result<PipelineData, ShellError>
        + 'static
        + Send
        + Sync,
> {
    Box::new(move |engine_state, stack, call, _input| {
        let span = call.head;
        let tool_name = tool.name.to_string();

        // Try to get the MCP client from the engine state
        let client = match super::utils::get_mcp_client(engine_state) {
            Ok(client) => client,
            Err(err) => {
                return Err(ShellError::GenericError {
                    error: "Could not access MCP client".into(),
                    msg: err.to_string(),
                    span: Some(span),
                    help: Some("Make sure the MCP client is connected".into()),
                    inner: Vec::new(),
                });
            }
        };

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
        let args_json = JsonValue::Object(params);

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
