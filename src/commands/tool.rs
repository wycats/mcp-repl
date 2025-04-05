use crate::client::mcp::{McpClient, Tool};
use crate::commands::utils;
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Span, SyntaxShape, Type, Value,
    ast::Call,
    engine::{Command, CommandArgs, EngineState, Stack},
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ToolCommand;

impl Command for ToolCommand {
    fn name(&self) -> &str {
        "tool"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool")
            .optional(
                "tool_name",
                SyntaxShape::String,
                "The name of the MCP tool to invoke",
            )
            .rest("args", SyntaxShape::Any, "Arguments to pass to the tool")
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Any, Type::Any)])
    }

    fn description(&self) -> &str {
        "Invoke an MCP tool with dynamic arguments"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;

        // Get the tool name from the first argument
        let tool_name: Option<String> = call.opt(engine_state, stack, 0)?;

        // Get the MCP client from engine state
        let client = utils::get_mcp_client(engine_state)?;

        if let Some(tool_name) = tool_name {
            // Get the tools registry
            let tools = utils::get_tools_registry(engine_state)?;
            let tools_guard = tools.lock().map_err(|_| ShellError::GenericError {
                error: "Failed to lock tools registry".into(),
                msg: "Internal synchronization error".into(),
                span: Some(span),
                help: None,
                inner: Vec::new(),
            })?;

            // Find the tool by name
            let tool = tools_guard.iter().find(|t| t.name == tool_name);

            if let Some(tool) = tool {
                // Build arguments based on schema
                let args = self.extract_tool_args(tool, engine_state, stack, call)?;

                // Create a tokio runtime for async operations
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| ShellError::GenericError {
                        error: "Failed to create Tokio runtime".into(),
                        msg: e.to_string(),
                        span: Some(span),
                        help: None,
                        inner: Vec::new(),
                    })?;

                // Execute the tool call
                let result = runtime.block_on(async {
                    let client_guard = client.lock().map_err(|_| ShellError::GenericError {
                        error: "Failed to lock MCP client".into(),
                        msg: "Internal synchronization error".into(),
                        span: Some(span),
                        help: None,
                        inner: Vec::new(),
                    })?;

                    if let Some(client) = &*client_guard {
                        client.call_tool(&tool.name, args).await
                    } else {
                        Err(ShellError::GenericError {
                            error: "MCP client not initialized".into(),
                            msg: "MCP client is not connected".into(),
                            span: Some(span),
                            help: Some("Connect to an MCP server first".into()),
                            inner: Vec::new(),
                        })
                    }
                })?;

                // Process the result
                process_tool_result(result, span)
            } else {
                Err(ShellError::GenericError {
                    error: format!("Unknown tool: {}", tool_name),
                    msg: "The specified tool was not found".into(),
                    span: Some(span),
                    help: Some("Use 'mcp-list-tools' to see available tools".into()),
                    inner: Vec::new(),
                })
            }
        } else {
            // No tool specified, show help with available tools
            let mut vals = Vec::new();
            let mut cols = vec!["name".to_string(), "description".to_string()];

            // Get available tools
            let tools = match utils::get_tools_registry(engine_state) {
                Ok(tools) => tools,
                Err(_) => {
                    return Err(ShellError::GenericError {
                        error: "No tools available".into(),
                        msg: "Tool registry not found".into(),
                        span: Some(span),
                        help: Some("Make sure the MCP client is connected".into()),
                        inner: Vec::new(),
                    });
                }
            };

            let tools_guard = tools.lock().map_err(|_| ShellError::GenericError {
                error: "Failed to lock tools registry".into(),
                msg: "Internal synchronization error".into(),
                span: Some(span),
                help: None,
                inner: Vec::new(),
            })?;

            // Format tools into a record for display
            for tool in tools_guard.iter() {
                vals.push(Value::String {
                    val: tool.name.to_string(),
                    span,
                });

                vals.push(Value::String {
                    val: tool.description.clone().unwrap_or_default(),
                    span,
                });
            }

            Ok(PipelineData::Value(
                Value::Record { cols, vals, span },
                None,
            ))
        }
    }
}

// Helper methods implemented separately
impl ToolCommand {
    // Extract and validate tool arguments from call
    fn extract_tool_args(
        &self,
        tool: &Tool,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
    ) -> Result<serde_json::Value, ShellError> {
        // Get rest arguments starting from index 1 (after tool name)
        let args = call.rest(engine_state, stack, 1)?;

        // For now, implement a simple conversion of arguments to JSON
        // This is a placeholder for more sophisticated schema-based extraction
        let mut params = serde_json::Map::new();

        // Process args in pairs (param_name, param_value)
        for chunk in args.chunks(2) {
            if chunk.len() == 2 {
                let param_name = match chunk[0].as_string() {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(ShellError::GenericError {
                            error: "Invalid parameter name".into(),
                            msg: "Parameter names must be strings".into(),
                            span: Some(chunk[0].span()?),
                            help: Some("Use 'param_name param_value' format".into()),
                            inner: Vec::new(),
                        });
                    }
                };

                // Convert Nu Value to serde_json::Value
                let param_value = nu_value_to_json_value(&chunk[1])?;
                params.insert(param_name, param_value);
            } else if chunk.len() == 1 {
                return Err(ShellError::GenericError {
                    error: "Incomplete parameter".into(),
                    msg: "Each parameter must have a value".into(),
                    span: Some(chunk[0].span()?),
                    help: Some("Use 'param_name param_value' format".into()),
                    inner: Vec::new(),
                });
            }
        }

        Ok(serde_json::Value::Object(params))
    }
}

// Helper function to convert Nu Value to serde_json::Value
fn nu_value_to_json_value(value: &Value) -> Result<serde_json::Value, ShellError> {
    let span = value.span()?;

    match value {
        Value::String { val, .. } => Ok(serde_json::Value::String(val.clone())),
        Value::Int { val, .. } => Ok(serde_json::Value::Number((*val).into())),
        Value::Float { val, .. } => {
            // Handle potential invalid float values for JSON
            if val.is_nan() || val.is_infinite() {
                Err(ShellError::GenericError {
                    error: "Invalid number".into(),
                    msg: "NaN or infinity cannot be represented in JSON".into(),
                    span: Some(span),
                    help: None,
                    inner: Vec::new(),
                })
            } else {
                // Convert to serde_json::Number
                match serde_json::Number::from_f64(*val) {
                    Some(n) => Ok(serde_json::Value::Number(n)),
                    None => Err(ShellError::GenericError {
                        error: "Failed to convert float".into(),
                        msg: "The float value could not be represented in JSON".into(),
                        span: Some(span),
                        help: None,
                        inner: Vec::new(),
                    }),
                }
            }
        }
        Value::Boolean { val, .. } => Ok(serde_json::Value::Bool(*val)),
        Value::List { vals, .. } => {
            let mut json_array = Vec::new();
            for item in vals {
                json_array.push(nu_value_to_json_value(item)?);
            }
            Ok(serde_json::Value::Array(json_array))
        }
        Value::Record { cols, vals, .. } => {
            let mut map = serde_json::Map::new();
            for (col, val) in cols.iter().zip(vals.iter()) {
                map.insert(col.clone(), nu_value_to_json_value(val)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Value::Nothing { .. } => Ok(serde_json::Value::Null),
        _ => Err(ShellError::GenericError {
            error: "Unsupported value type".into(),
            msg: "This value type cannot be converted to JSON".into(),
            span: Some(span),
            help: None,
            inner: Vec::new(),
        }),
    }
}

// Process MCP tool results into Nushell values
fn process_tool_result(
    content: Vec<rmcp::model::Content>,
    span: Span,
) -> Result<PipelineData, ShellError> {
    // Convert rmcp::model::Content to Value
    // For now, return a simple format
    let mut rows = Vec::new();

    for item in content {
        // Convert each content item to a Nu value
        // This is a simplified implementation
        // Could be expanded based on different content types
        match item {
            rmcp::model::Content::Text(text) => {
                rows.push(Value::String { val: text, span });
            }
            // Handle other content types here
            _ => {
                // For now, convert other types to their debug representation
                rows.push(Value::String {
                    val: format!("{:?}", item),
                    span,
                });
            }
        }
    }

    // If we have multiple results, return as a list
    if rows.len() > 1 {
        Ok(PipelineData::Value(Value::List { vals: rows, span }, None))
    } else if rows.len() == 1 {
        // If we have exactly one result, return it directly
        Ok(PipelineData::Value(rows.into_iter().next().unwrap(), None))
    } else {
        // If we have no results, return Nothing
        Ok(PipelineData::Value(Value::Nothing { span }, None))
    }
}
