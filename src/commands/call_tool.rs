use nu_engine::CallExt;

use nu_protocol::{
    Category, PipelineData, Record, ShellError, Signature, Span, SyntaxShape, Value,
    ast::PathMember,
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
        let json_args = if let Some(args) = args {
            if let Ok(record) = args.as_record() {
                let mut map = serde_json::Map::new();
                for (k, v) in record.iter() {
                    map.insert(k.clone(), convert_nu_value_to_json_value(v, span)?);
                }
                Some(map)
            } else {
                return Err(ShellError::GenericError {
                    error: "Arguments must be a record".into(),
                    msg: format!("Got {} instead", args.get_type()),
                    span: Some(span),
                    help: None,
                    inner: vec![],
                });
            }
        } else {
            None
        };

        // Try to get the MCP client from the utils
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

pub fn convert_json_value_to_nu_value(
    v: &serde_json::Value,
    span: Span,
) -> Result<Value, ShellError> {
    let result = match v {
        serde_json::Value::Null => Value::Nothing {
            internal_span: span,
        },
        serde_json::Value::Bool(b) => Value::Bool {
            val: *b,
            internal_span: span,
        },
        serde_json::Value::Number(n) => {
            if let Some(val) = n.as_i64() {
                Value::Int {
                    val,
                    internal_span: span,
                }
            } else if let Some(val) = n.as_f64() {
                Value::Float {
                    val,
                    internal_span: span,
                }
            } else {
                return Err(crate::util::error::generic_error(
                    format!(
                        "Unexpected numeric value, cannot convert {} into i64 or f64",
                        n
                    ),
                    None,
                    None,
                ));
            }
        }
        serde_json::Value::String(val) => Value::String {
            val: val.clone(),
            internal_span: span,
        },
        serde_json::Value::Array(a) => {
            let t = a
                .iter()
                .map(|x| convert_json_value_to_nu_value(x, span))
                .collect::<Result<Vec<Value>, ShellError>>()?;
            Value::List {
                vals: t,
                internal_span: span,
            }
        }
        serde_json::Value::Object(o) => {
            let mut cols = vec![];
            let mut vals = vec![];

            for (k, v) in o.iter() {
                cols.push(k.clone());
                vals.push(convert_json_value_to_nu_value(v, span)?);
            }

            Value::Record {
                val: nu_utils::SharedCow::new(
                    Record::from_raw_cols_vals(cols, vals, span, span).unwrap(),
                ),
                internal_span: span,
            }
        }
    };

    Ok(result)
}

// Adapted from https://github.com/nushell/nushell/blob/main/crates/nu-command/src/commands/formats/to/json.rs
pub fn convert_nu_value_to_json_value(
    v: &Value,
    span: Span,
) -> Result<serde_json::Value, ShellError> {
    Ok(match v {
        Value::Bool { val, .. } => serde_json::Value::Bool(*val),
        Value::Filesize { val, .. } => {
            serde_json::Value::Number(serde_json::Number::from(val.get()))
        }
        Value::Duration { val, .. } => serde_json::Value::String(val.to_string()),
        Value::Date { val, .. } => serde_json::Value::String(val.to_string()),
        Value::Float { val, .. } => {
            if let Some(num) = serde_json::Number::from_f64(*val) {
                serde_json::Value::Number(num)
            } else {
                return Err(crate::util::error::generic_error(
                    format!("Unexpected numeric value, cannot convert {} from f64", val),
                    None,
                    None,
                ));
            }
        }
        Value::Int { val, .. } => serde_json::Value::Number(serde_json::Number::from(*val)),
        Value::Nothing { .. } => serde_json::Value::Null,
        Value::String { val, .. } => serde_json::Value::String(val.clone()),
        Value::CellPath { val, .. } => serde_json::Value::Array(
            val.members
                .iter()
                .map(|x| match &x {
                    PathMember::String { val, .. } => Ok(serde_json::Value::String(val.clone())),
                    PathMember::Int { val, .. } => Ok(serde_json::Value::Number(
                        serde_json::Number::from(*val as u64),
                    )),
                })
                .collect::<Result<Vec<serde_json::Value>, ShellError>>()?,
        ),
        Value::List { vals, .. } => serde_json::Value::Array(json_list(vals, span)?),
        Value::Error { error, .. } => return Err(*error.clone()),
        Value::Binary { val, .. } => serde_json::Value::Array(
            val.iter()
                .map(|x| {
                    Ok(serde_json::Value::Number(serde_json::Number::from(
                        *x as u64,
                    )))
                })
                .collect::<Result<Vec<serde_json::Value>, ShellError>>()?,
        ),
        Value::Record { val, .. } => {
            let mut m = serde_json::Map::new();
            for (k, v) in val.iter() {
                m.insert(k.clone(), convert_nu_value_to_json_value(v, span)?);
            }
            serde_json::Value::Object(m)
        }
        Value::Custom { .. } => serde_json::Value::Null,
        Value::Range { .. } => serde_json::Value::Null,
        Value::Closure { .. } => serde_json::Value::Null,
        Value::Glob { val, .. } => serde_json::Value::String(val.clone()),
    })
}

fn json_list(input: &[Value], span: Span) -> Result<Vec<serde_json::Value>, ShellError> {
    let mut out = vec![];

    for value in input {
        out.push(convert_nu_value_to_json_value(value, span)?);
    }

    Ok(out)
}
