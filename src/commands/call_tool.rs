use nu_engine::CallExt;
use nu_json::value::ToJson;
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Span, SyntaxShape, Value,
    engine::{Command, EngineState, Stack},
};
use rmcp::model::RawContent;
use serde_json::json;

/// Call an MCP tool command
#[derive(Clone)]
pub struct CallToolCommand;

impl Command for CallToolCommand {
    fn name(&self) -> &str {
        "mcp-call-tool"
    }

    fn signature(&self) -> Signature {
        Signature::build("mcp-call-tool")
            .required("name", SyntaxShape::String, "Name of the tool to call")
            .optional(
                "args",
                nu_protocol::SyntaxShape::Record(vec![]),
                "Arguments to pass to the tool (as a record)",
            )
            .category(Category::Custom("mcp".to_string()))
    }

    fn description(&self) -> &str {
        "Call an MCP tool with the given arguments"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &nu_protocol::engine::Call<'_>,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;

        // Get the tool name
        let tool_name: String = call.req(engine_state, stack, 0)?;

        // Get the arguments (optional)
        let args = call.opt::<Value>(engine_state, stack, 1)?;

        // Convert args to serde_json::Value if present
        let json_args = match args {
            Some(val) => {
                if let Ok(record) = val.as_record() {
                    let mut map = serde_json::Map::new();

                    for (k, v) in record.iter() {
                        map.insert(k.clone(), nu_value_to_json_value(v, span)?);
                    }

                    Some(map)
                } else {
                    return Err(ShellError::GenericError {
                        error: "Arguments must be a record".into(),
                        msg: "Expected a record of key-value pairs".into(),
                        span: Some(span),
                        help: Some("Example: { key: value }".into()),
                        inner: Vec::new(),
                    });
                }
            }
            Some(_) => {
                return Err(ShellError::GenericError {
                    error: "Arguments must be a record".into(),
                    msg: "Expected a record of key-value pairs".into(),
                    span: Some(span),
                    help: Some("Example: { key: value }".into()),
                    inner: Vec::new(),
                });
            }
            None => None,
        };

        // Try to get the MCP client from the utils
        let client = match super::utils::get_mcp_client(stack) {
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

        // Execute the call_tool method using the MCP client
        let result = runtime.block_on(async {
            // Create the arguments JSON value
            let args_json = match json_args {
                Some(map) => serde_json::Value::Object(map),
                None => serde_json::Value::Null,
            };

            // Call the tool through the MCP client
            client.call_tool(&tool_name, args_json).await
        });

        match result {
            Ok(tool_result) => {
                // Convert the tool result to Nushell values
                let mut results = Vec::new();

                for content in tool_result {
                    match content.raw {
                        RawContent::Text(text) => {
                            results.push(Value::string(text.text, span));
                        }
                        RawContent::Image(image) => {
                            results.push(Value::string(image.data, span));
                        }
                        RawContent::Resource(resource) => {
                            // Match on the ResourceContents variants
                            match &resource.resource {
                                rmcp::model::ResourceContents::TextResourceContents {
                                    text,
                                    ..
                                } => {
                                    results.push(Value::string(text, span));
                                }
                                rmcp::model::ResourceContents::BlobResourceContents {
                                    blob,
                                    ..
                                } => {
                                    results.push(Value::string(blob, span));
                                }
                            }
                        }
                    }
                }

                // Return as a list if multiple results, otherwise just the single value
                let result_value = if results.len() == 1 {
                    results.remove(0)
                } else {
                    Value::list(results, span)
                };

                Ok(PipelineData::Value(result_value, None))
            }
            Err(e) => Err(ShellError::GenericError {
                error: "Failed to call tool".into(),
                msg: e.to_string(),
                span: Some(span),
                help: None,
                inner: Vec::new(),
            }),
        }
    }
}

// Convert a Nushell Value to a serde_json::Value
fn nu_value_to_json_value(value: &Value, span: Span) -> Result<serde_json::Value, ShellError> {
    match value {
        Value::String { val, .. } => Ok(json!(val)),
        Value::Int { val, .. } => Ok(json!(val)),
        Value::Float { val, .. } => Ok(json!(val)),
        Value::Bool { val, .. } => Ok(json!(val)),
        Value::Nothing { .. } => Ok(serde_json::Value::Null),
        Value::Record { .. } => {
            let mut obj = serde_json::Map::new();

            // In Nushell 0.103.0, we need to use as_record() and then iterate through it
            if let Ok(record) = value.as_record() {
                for (k, v) in record.iter() {
                    obj.insert(k.clone(), nu_value_to_json_value(v, span)?);
                }
            }

            Ok(serde_json::Value::Object(obj))
        }
        Value::List { vals, .. } => {
            let mut arr = Vec::new();

            for v in vals {
                arr.push(nu_value_to_json_value(v, span)?);
            }

            Ok(serde_json::Value::Array(arr))
        }
        _ => Err(ShellError::GenericError {
            error: "Unsupported value type".into(),
            msg: format!("Cannot convert {:?} to JSON", value),
            span: Some(span),
            help: None,
            inner: Vec::new(),
        }),
    }
}

// Convert a serde_json::Value to a Nushell Value
fn json_value_to_nu_value(json: &serde_json::Value, span: Span) -> Result<Value, ShellError> {
    match json {
        serde_json::Value::Null => Ok(Value::nothing(span)),
        serde_json::Value::Bool(b) => Ok(Value::bool(*b, span)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::int(i, span))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::float(f, span))
            } else {
                Err(ShellError::GenericError {
                    error: "Unsupported number type".into(),
                    msg: format!("Cannot convert number {:?} to Nushell value", n),
                    span: Some(span),
                    help: None,
                    inner: Vec::new(),
                })
            }
        }
        serde_json::Value::String(s) => Ok(Value::string(s.clone(), span)),
        serde_json::Value::Array(arr) => {
            let mut values = Vec::new();

            for v in arr {
                values.push(json_value_to_nu_value(v, span)?);
            }

            Ok(Value::list(values, span))
        }
        serde_json::Value::Object(obj) => {
            // Create a Record to hold key-value pairs
            let mut record = nu_protocol::Record::new();

            for (k, v) in obj {
                record.insert(k.clone(), json_value_to_nu_value(v, span)?);
            }

            // Use the current Nushell Value::record constructor
            Ok(Value::record(record, span))
        }
    }
}
