use anyhow::Result;
use nu_engine::CallExt;
use nu_protocol::{
    Category, Signature, SyntaxShape, Type,
    engine::{EngineState, Stack},
};
use rmcp::model::Tool;
use serde_json::Value as JsonValue;

/// Maps an MCP tool to a Nushell command signature
pub fn map_tool_to_signature(tool: &Tool, category: &str) -> Result<Signature> {
    let name = tool.name.to_string();
    let mut signature =
        Signature::build(name.clone()).category(Category::Custom(category.to_string()));

    // Handle schema parameters
    if let Some(schema_props) = get_schema_properties(tool) {
        for (param_name, param_schema) in schema_props {
            // Determine if parameter is required
            let is_required = is_parameter_required(tool, &param_name)?;

            // Get parameter description
            let description = get_parameter_description(&param_schema)
                .unwrap_or_else(|| format!("{} parameter", param_name));

            // Determine parameter type/shape
            let syntax_shape = map_json_schema_to_syntax_shape(&param_schema)?;

            // Add parameter to signature
            if is_required {
                signature = signature.required(param_name, syntax_shape, description);
            } else {
                signature = signature.optional(param_name, syntax_shape, description);
            }
        }
    }

    Ok(signature)
}

/// Get properties from a JSON Schema
fn get_schema_properties(tool: &Tool) -> Option<serde_json::Map<String, JsonValue>> {
    let schema = tool.schema_as_json_value();

    if let JsonValue::Object(obj) = schema {
        if let Some(JsonValue::Object(properties)) = obj.get("properties") {
            return Some(properties.clone());
        }
    }

    None
}

/// Check if a parameter is required in the JSON Schema
fn is_parameter_required(tool: &Tool, param_name: &str) -> Result<bool> {
    let schema = tool.schema_as_json_value();

    if let JsonValue::Object(obj) = schema {
        if let Some(JsonValue::Array(required)) = obj.get("required") {
            return Ok(required.iter().any(|value| {
                if let JsonValue::String(name) = value {
                    name == param_name
                } else {
                    false
                }
            }));
        }
    }

    Ok(false)
}

/// Extract description from a parameter schema
fn get_parameter_description(param_schema: &JsonValue) -> Option<String> {
    if let JsonValue::Object(obj) = param_schema {
        if let Some(JsonValue::String(desc)) = obj.get("description") {
            return Some(desc.clone());
        }
    }

    None
}

/// Map JSON Schema types to Nushell syntax shapes
fn map_json_schema_to_syntax_shape(param_schema: &JsonValue) -> Result<SyntaxShape> {
    if let JsonValue::Object(obj) = param_schema {
        // Get the type field from the schema
        if let Some(JsonValue::String(type_str)) = obj.get("type") {
            match type_str.as_str() {
                "string" => {
                    // Check if it's an enum
                    if obj.contains_key("enum") {
                        return Ok(SyntaxShape::String); // For enums, we still use string shape
                    }

                    // Check for format specifiers
                    if let Some(JsonValue::String(format)) = obj.get("format") {
                        match format.as_str() {
                            "date-time" => return Ok(SyntaxShape::DateTime),
                            "date" => return Ok(SyntaxShape::DateTime),
                            "time" => return Ok(SyntaxShape::DateTime),
                            "uri" => return Ok(SyntaxShape::String),
                            "email" => return Ok(SyntaxShape::String),
                            "uuid" => return Ok(SyntaxShape::String),
                            _ => return Ok(SyntaxShape::String),
                        }
                    }

                    return Ok(SyntaxShape::String);
                }
                "number" => Ok(SyntaxShape::Number),
                "integer" => Ok(SyntaxShape::Int),
                "boolean" => Ok(SyntaxShape::Boolean),
                "array" => {
                    // Check if it has items specification
                    if let Some(items) = obj.get("items") {
                        if let Ok(item_shape) = map_json_schema_to_syntax_shape(items) {
                            // Use Table for complex types, List for simpler types
                            match item_shape {
                                SyntaxShape::Record(_) => {
                                    // Create an empty Table syntax shape with no fields
                                    return Ok(SyntaxShape::Table(Vec::new()));
                                }
                                _ => return Ok(SyntaxShape::List(Box::new(item_shape))),
                            }
                        }
                    }

                    // Default to list of any
                    Ok(SyntaxShape::List(Box::new(SyntaxShape::Any)))
                }
                "object" => {
                    // For objects with defined properties, use Record
                    if obj.contains_key("properties") {
                        return Ok(SyntaxShape::Record(vec![]));
                    }

                    // For generic objects, use Any
                    Ok(SyntaxShape::Any)
                }
                "null" => Ok(SyntaxShape::Nothing),
                _ => Ok(SyntaxShape::Any), // Default to Any for unknown types
            }
        } else if obj.contains_key("oneOf")
            || obj.contains_key("anyOf")
            || obj.contains_key("allOf")
        {
            // For complex schemas with oneOf/anyOf/allOf, default to Any
            Ok(SyntaxShape::Any)
        } else {
            // Default to Any if no type is specified
            Ok(SyntaxShape::Any)
        }
    } else {
        // Default to Any for non-object schemas
        Ok(SyntaxShape::Any)
    }
}

/// Generate a help description from an MCP tool
pub fn generate_help_description(tool: &Tool) -> String {
    match &tool.description {
        Some(desc) => desc.to_string(),
        None => format!("MCP tool: {}", tool.name),
    }
}

/// Convert MCP tool parameters to a Nushell input_output_types specification
pub fn generate_input_output_types(_tool: &Tool) -> Vec<(Type, Type)> {
    // Most MCP tools take no pipeline input and return a string
    // This is a simplification - could be enhanced with actual schema analysis
    vec![(Type::Nothing, Type::String)]
}

/// Map Nushell values to JSON values for tool parameters
pub fn map_call_args_to_tool_params(
    engine_state: &EngineState,
    stack: &mut Stack,
    call: &nu_protocol::engine::Call<'_>,
    tool: &Tool,
) -> Result<serde_json::Map<String, JsonValue>> {
    let mut params = serde_json::Map::new();
    let span = call.head;
    
    // Get schema properties from the tool
    if let Some(properties) = get_schema_properties(tool) {
        let mut prop_vec: Vec<(String, JsonValue)> = properties.into_iter().collect();
        
        // Sort properties so required ones are first (helps with positional args mapping)
        prop_vec.sort_by(|(name1, _), (name2, _)| {
            let req1 = is_parameter_required(tool, name1).unwrap_or(false);
            let req2 = is_parameter_required(tool, name2).unwrap_or(false);
            req2.cmp(&req1) // required first
        });
        
        // First, process positional arguments
        let mut positional_idx = 0;
        for (param_name, param_schema) in &prop_vec {
            // Check if this is a required parameter (likely to be positional)
            let is_required = is_parameter_required(tool, param_name).unwrap_or(false);
            
            // If this is a required parameter, try to get it as a positional argument
            if is_required {
                // For positional arguments, we use the CallExt trait methods like req/opt
                // For the first argument, use index 0, second argument index 1, etc.
                let value_result = match positional_idx {
                    0 => call.opt(engine_state, stack, 0),
                    1 => call.opt(engine_state, stack, 1),
                    2 => call.opt(engine_state, stack, 2),
                    _ => Ok(None), // Support up to 3 positional arguments for now
                };
                
                if let Ok(Some(value)) = value_result {
                    let json_value = super::call_tool::convert_nu_value_to_json_value(&value, span)?;
                    params.insert(param_name.clone(), json_value);
                    positional_idx += 1;
                    continue; // Skip to next parameter
                }
            }
            
            // Try to get the parameter from flags
            if let Some(value) = call.get_flag(engine_state, stack, param_name)? {
                let json_value = super::call_tool::convert_nu_value_to_json_value(&value, span)?;
                params.insert(param_name.clone(), json_value);
            }
        }
    }
    
    Ok(params)
}
