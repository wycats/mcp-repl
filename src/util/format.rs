use anyhow::{Result, anyhow};
use nu_protocol::{IntoPipelineData, PipelineData, Span, Value};
use serde_json::Value as JsonValue;

/// Convert a JSON value to a Nushell value for pretty display
pub fn json_to_nu_value(json: &JsonValue, span: Span) -> Result<Value> {
    // Use the existing conversion function from call_tool
    crate::commands::call_tool::convert_json_value_to_nu_value(json, span)
        .map_err(|e| anyhow!(e.to_string()))
}

/// Format a JSON value as a string using Nushell's value rendering
pub fn format_json_as_nu(json: &JsonValue, span: Span) -> String {
    match json_to_nu_value(json, span) {
        Ok(nu_value) => {
            // Try to convert the value to a string using Nushell's own string representation
            // In Nushell 0.103.0, use coerce_into_string which tries to represent any value as a string
            match nu_value.clone().coerce_into_string() {
                Ok(s) => s,
                Err(_) => {
                    // If direct conversion fails, try to use the actual value display logic
                    // by going through into_pipeline_data which is what Nushell uses in the REPL
                    match nu_value.clone().into_pipeline_data() {
                        PipelineData::Value(val, ..) => {
                            match val.coerce_into_string() {
                                Ok(s) => s,
                                Err(_) => format_nu_value(&nu_value), // Fallback to our custom formatter
                            }
                        }
                        _ => format_nu_value(&nu_value),
                    }
                }
            }
        }
        Err(_) => format!("{:#}", json), // Fallback to regular JSON formatting
    }
}

/// Format a Nushell value as a string (fallback for simple values)
pub fn format_nu_value(value: &Value) -> String {
    match value {
        Value::String { val, .. } => format!("{}", val),
        Value::Int { val, .. } => format!("{}", val),
        Value::Float { val, .. } => format!("{}", val),
        Value::Bool { val, .. } => format!("{}", val),
        Value::Date { val, .. } => format!("{}", val),
        Value::Duration { val, .. } => format!("{}", val),
        Value::Nothing { .. } => "null".to_string(),
        Value::List { vals, .. } => {
            if vals.is_empty() {
                "[]".to_string()
            } else {
                let items: Vec<String> = vals.iter().map(format_nu_value).collect();
                format!("[{}]", items.join(", "))
            }
        }
        Value::Record { val, .. } => {
            if val.is_empty() {
                "{}".to_string()
            } else {
                let mut items = vec![];
                for (key, value) in val.iter() {
                    items.push(format!("{}: {}", key, format_nu_value(value)));
                }
                format!("{{{}}}", items.join(", "))
            }
        }
        _ => format!("{:?}", value),
    }
}

/// Convert a JSON object to a Nushell record and format it as a table string
pub fn format_json_object_as_table(
    json_obj: &serde_json::Map<String, serde_json::Value>,
    span: Span,
) -> String {
    match serde_json::Value::Object(json_obj.clone()) {
        json => format_json_as_nu(&json, span),
    }
}

/// Convert a Nushell Value to PipelineData for display
pub fn nu_value_to_pipeline_data(value: Value) -> PipelineData {
    value.into_pipeline_data()
}
