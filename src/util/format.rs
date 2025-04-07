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

#[cfg(test)]
mod tests {
    use super::*;
    use nu_protocol::record;
    use serde_json::json;

    fn test_span() -> Span {
        Span::unknown()
    }

    #[test]
    fn test_format_nu_value_primitives() {
        // Test string formatting
        let string_val = Value::string("hello world", test_span());
        assert_eq!(format_nu_value(&string_val), "hello world");

        // Test integer formatting
        let int_val = Value::int(42, test_span());
        assert_eq!(format_nu_value(&int_val), "42");

        // Test float formatting
        let float_val = Value::float(3.14, test_span());
        assert_eq!(format_nu_value(&float_val), "3.14");

        // Test boolean formatting
        let bool_val = Value::bool(true, test_span());
        assert_eq!(format_nu_value(&bool_val), "true");

        // Test nothing formatting
        let nothing_val = Value::nothing(test_span());
        assert_eq!(format_nu_value(&nothing_val), "null");
    }

    #[test]
    fn test_format_nu_value_collections() {
        // Test empty list
        let empty_list = Value::list(vec![], test_span());
        assert_eq!(format_nu_value(&empty_list), "[]");

        // Test list with elements
        let list = Value::list(
            vec![
                Value::int(1, test_span()),
                Value::int(2, test_span()),
                Value::int(3, test_span()),
            ],
            test_span(),
        );
        assert_eq!(format_nu_value(&list), "[1, 2, 3]");

        // Test nested list
        let nested_list = Value::list(
            vec![
                Value::int(1, test_span()),
                Value::list(
                    vec![Value::int(2, test_span()), Value::int(3, test_span())],
                    test_span(),
                ),
            ],
            test_span(),
        );
        assert_eq!(format_nu_value(&nested_list), "[1, [2, 3]]");

        // Test empty record
        let empty_record = Value::record(record! {}, test_span());
        assert_eq!(format_nu_value(&empty_record), "{}");

        // Test record with values
        let record_val = Value::record(
            record! {
                "name" => Value::string("John", test_span()),
                "age" => Value::int(30, test_span()),
            },
            test_span(),
        );
        // Order might vary, so check if both fields are present
        let formatted = format_nu_value(&record_val);
        assert!(formatted.contains("name: John"));
        assert!(formatted.contains("age: 30"));
        assert!(formatted.starts_with("{"));
        assert!(formatted.ends_with("}"));
    }

    #[test]
    fn test_format_json_as_nu() {
        // Test simple string
        let json_string = json!("hello");
        assert_eq!(format_json_as_nu(&json_string, test_span()), "hello");

        // Test integer
        let json_int = json!(42);
        assert_eq!(format_json_as_nu(&json_int, test_span()), "42");

        // Test object
        let json_obj = json!({
            "name": "Alice",
            "age": 25
        });
        let formatted = format_json_as_nu(&json_obj, test_span());
        assert!(formatted.contains("name"));
        assert!(formatted.contains("Alice"));
        assert!(formatted.contains("age"));
        assert!(formatted.contains("25"));
    }

    #[test]
    fn test_format_json_object_as_table() {
        // Create a JSON object
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), json!("Bob"));
        obj.insert("age".to_string(), json!(35));
        obj.insert("is_active".to_string(), json!(true));

        let formatted = format_json_object_as_table(&obj, test_span());

        // Verify all values are present
        assert!(formatted.contains("name"));
        assert!(formatted.contains("Bob"));
        assert!(formatted.contains("age"));
        assert!(formatted.contains("35"));
        assert!(formatted.contains("is_active"));
        assert!(formatted.contains("true"));
    }

    #[test]
    fn test_json_to_nu_value() {
        // Since this function uses an external converter, we just test some basics
        // assuming the converter works correctly

        // Test null
        let json_null = json!(null);
        let nu_null = json_to_nu_value(&json_null, test_span()).unwrap();
        assert!(matches!(nu_null, Value::Nothing { .. }));

        // Test simple values
        let json_string = json!("test");
        let nu_string = json_to_nu_value(&json_string, test_span()).unwrap();
        if let Value::String { val, .. } = nu_string {
            assert_eq!(val, "test");
        } else {
            panic!("Expected String, got {:?}", nu_string);
        }

        let json_number = json!(123);
        let nu_number = json_to_nu_value(&json_number, test_span()).unwrap();
        if let Value::Int { val, .. } = nu_number {
            assert_eq!(val, 123);
        } else {
            panic!("Expected Int, got {:?}", nu_number);
        }
    }

    #[test]
    fn test_nu_value_to_pipeline_data() {
        // Test conversion to pipeline data
        let value = Value::string("hello pipeline", test_span());
        let pipeline = nu_value_to_pipeline_data(value);

        // Verify it's a PipelineData with the correct value
        if let PipelineData::Value(val, ..) = pipeline {
            if let Value::String {
                val: string_val, ..
            } = val
            {
                assert_eq!(string_val, "hello pipeline");
            } else {
                panic!("Expected String, got {:?}", val);
            }
        } else {
            panic!("Expected PipelineData::Value, got something else");
        }
    }
}
