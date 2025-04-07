use anyhow::Result;
use nu_engine::CallExt;
use nu_protocol::{
    Category, Signature, SyntaxShape, Type,
    engine::{EngineState, Stack},
};
use rmcp::model::Tool;
use serde_json::Value as JsonValue;

/// Maps an MCP tool to a Nushell command signature
/// Following the mapping strategy in MAPPING.md:
/// 1. If the tool has exactly one required or optional parameter, map it onto a positional argument.
/// 2. If the tool has exactly two required parameters, map them onto positional arguments.
/// 3. If the tool has exactly one or two required parameters and all of the rest of the arguments are optional, map the required parameters onto positional arguments and the optional parameters onto flags.
/// 4. Optional parameters that are booleans should be mapped to switches (e.g., `--verbose`).
/// 5. All other optional parameters should be mapped to flags (e.g., `--limit 10`).
pub fn map_tool_to_signature(tool: &Tool, category: &str) -> Result<Signature> {
    let name = tool.name.to_string();

    // DEBUG: Output the raw schema for inspection
    eprintln!("DEBUG: Tool {} schema: {:?}", name, tool.input_schema);

    let mut signature =
        Signature::build(name.clone()).category(Category::Custom(category.to_string()));

    // Get all schema properties
    if let Some(schema_props) = get_schema_properties(tool) {
        // DEBUG: Output the properties we found
        eprintln!(
            "DEBUG: Properties for tool {}: {:?}",
            name,
            schema_props.keys().collect::<Vec<_>>()
        );

        // Convert properties to vec for sorting
        let prop_vec: Vec<(String, JsonValue)> = schema_props.into_iter().collect();

        // Identify required and optional parameters
        let required_params: Vec<(String, JsonValue)> = prop_vec
            .iter()
            .filter(|(name, _)| is_parameter_required(tool, name).unwrap_or(false))
            .map(|(name, schema)| (name.clone(), schema.clone()))
            .collect();
            
        let optional_params: Vec<(String, JsonValue)> = prop_vec
            .iter()
            .filter(|(name, _)| !is_parameter_required(tool, name).unwrap_or(true))
            .map(|(name, schema)| (name.clone(), schema.clone()))
            .collect();

        // Determine positional parameters based on the new rules
        let total_param_count = prop_vec.len();
        let positional_count = if total_param_count == 1 {
            // Rule 1: If exactly one parameter (required or optional), make it positional
            1
        } else if required_params.len() == 2 {
            // Rule 2: If exactly two required parameters, make them positional
            2
        } else if required_params.len() == 1 && !optional_params.is_empty() {
            // Rule 3: If exactly one required parameter and rest are optional, 
            // make the required one positional
            1
        } else {
            // Default to no positional parameters for other cases
            0
        };

        // Process positional parameters first based on our rules
        for i in 0..positional_count {
            let param_name: &str;
            let param_schema: &JsonValue;
            
            // For tools with a single parameter (required or optional)
            if total_param_count == 1 {
                let (name, schema) = &prop_vec[0];
                param_name = name;
                param_schema = schema;
            } else if i < required_params.len() {
                // Required parameters get priority for positional slots
                let (name, schema) = &required_params[i];
                param_name = name;
                param_schema = schema;

            // Get parameter description
            let description = get_parameter_description(param_schema)
                .unwrap_or_else(|| format!("{} parameter", param_name));

            // Determine parameter type/shape
            let syntax_shape = map_json_schema_to_syntax_shape(param_schema)?;

                // Determine if parameter is required or optional
                let is_required = is_parameter_required(tool, param_name)?;
                
                if is_required {
                    // Add as required positional parameter
                    signature = signature.required(param_name.clone(), syntax_shape, description);
                } else {
                    // Add as optional positional parameter
                    signature = signature.optional(param_name.clone(), syntax_shape, description);
                }
            }
        }

        // Process remaining parameters as flags
        for (param_name, param_schema) in prop_vec {
            // Skip parameters we've already processed as positional
            if positional_count > 0
                && required_params
                    .iter()
                    .take(positional_count)
                    .any(|(name, _)| name == &param_name)
            {
                continue;
            }

            // Get parameter description with better fallback
            let description = get_parameter_description(&param_schema)
                .or_else(|| {
                    // If no description found, extract useful information from schema
                    extract_useful_schema_info(&param_schema, &param_name)
                })
                .unwrap_or_else(|| format!("{} parameter", param_name));

            // Determine parameter type/shape
            let syntax_shape = map_json_schema_to_syntax_shape(&param_schema)?;

            // Determine if parameter is required
            let is_required = is_parameter_required(tool, &param_name)?;

            // Handle boolean parameters as switches if optional
            if !is_required && is_boolean_parameter(&param_schema) {
                // For boolean optional parameters, use switch (--param_name with no value)
                signature = signature.switch(param_name.clone(), description, None);
            } else if is_required {
                // For required parameters beyond the first 2, use flags with named parameters
                signature = signature.named(
                    param_name.clone(),
                    syntax_shape,
                    description,
                    None, // No short flag
                );
            } else {
                // Optional non-boolean - add as optional flag with named parameters
                signature = signature.named(
                    param_name.clone(),
                    syntax_shape,
                    description,
                    None, // No short flag
                );
            }
        }
    }

    Ok(signature)
}

/// Check if a parameter is a boolean type
fn is_boolean_parameter(param_schema: &JsonValue) -> bool {
    if let JsonValue::Object(obj) = param_schema {
        if let Some(JsonValue::String(type_str)) = obj.get("type") {
            return type_str == "boolean";
        }
    }
    false
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
        // First try to get the description directly
        if let Some(JsonValue::String(desc)) = obj.get("description") {
            return Some(desc.clone());
        }
    }

    // If we don't find a description, return None and let the caller handle the fallback
    None
}

/// Extract useful information from the schema when no description is available
fn extract_useful_schema_info(param_schema: &JsonValue, param_name: &str) -> Option<String> {
    if let JsonValue::Object(obj) = param_schema {
        // Check if we have enum values (choices) - this should be highest priority
        if let Some(JsonValue::Array(enum_values)) = obj.get("enum") {
            let values: Vec<String> = enum_values
                .iter()
                .filter_map(|v| {
                    if let JsonValue::String(s) = v {
                        Some(format!("\"{}\"" , s.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            if !values.is_empty() {
                return Some(format!("Valid values: {}", values.join(", ")));
            }
        }

        // Check if we have format information
        if let Some(JsonValue::String(format)) = obj.get("format") {
            return Some(format!("{} in {} format", param_name, format));
        }

        // Check for pattern (regex)
        if let Some(JsonValue::String(pattern)) = obj.get("pattern") {
            return Some(format!("Must match pattern: {}", pattern));
        }

        // Check for min/max constraints
        let mut constraints = Vec::new();

        if let Some(JsonValue::Number(min)) = obj.get("minimum") {
            constraints.push(format!("min: {}", min));
        }

        if let Some(JsonValue::Number(max)) = obj.get("maximum") {
            constraints.push(format!("max: {}", max));
        }

        if !constraints.is_empty() {
            return Some(format!("Constraints: {}", constraints.join(", ")));
        }

        // Check if it's an object and describe its structure
        if let Some(JsonValue::String(type_str)) = obj.get("type") {
            if type_str == "object" {
                return Some("JSON object parameter".to_string());
            } else if type_str == "array" {
                return Some("List of values".to_string());
            }
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
                        // Use String for enums
                        // The parameter description will include detailed information
                        // about valid values for better documentation
                        return Ok(SyntaxShape::String);
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
/// Following the mapping strategy in MAPPING.md:
/// 1. If the tool has exactly one required or optional parameter, map it onto a positional argument.
/// 2. If the tool has exactly two required parameters, map them onto positional arguments.
/// 3. If the tool has exactly one or two required parameters and all of the rest of the arguments are optional, map the required parameters onto positional arguments and the optional parameters onto flags.
/// 4. Optional parameters that are booleans should be mapped to switches (e.g., `--verbose`).
/// 5. All other optional parameters should be mapped to flags (e.g., `--limit 10`).
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

        // Identify required and optional parameters
        let required_params: Vec<(String, JsonValue)> = prop_vec
            .iter()
            .filter(|(name, _)| is_parameter_required(tool, name).unwrap_or(false))
            .map(|(name, schema)| (name.clone(), schema.clone()))
            .collect();
            
        let optional_params: Vec<(String, JsonValue)> = prop_vec
            .iter()
            .filter(|(name, _)| !is_parameter_required(tool, name).unwrap_or(true))
            .map(|(name, schema)| (name.clone(), schema.clone()))
            .collect();

        // Determine positional parameters based on the new rules
        let total_param_count = prop_vec.len();
        let positional_count = if total_param_count == 1 {
            // Rule 1: If exactly one parameter (required or optional), make it positional
            1
        } else if required_params.len() == 2 {
            // Rule 2: If exactly two required parameters, make them positional
            2
        } else if required_params.len() == 1 && !optional_params.is_empty() {
            // Rule 3: If exactly one required parameter and rest are optional, 
            // make the required one positional
            1
        } else {
            // Default to no positional parameters for other cases
            0
        };

        // Process positional parameters based on our rules
        for i in 0..positional_count {
            let param_name: &str;
            
            // For tools with a single parameter (required or optional)
            if total_param_count == 1 {
                let (name, _) = &prop_vec[0];
                param_name = name;
            } else if i < required_params.len() {
                // Required parameters get priority for positional slots
                let (name, _) = &required_params[i];
                param_name = name;
            } else {
                // This shouldn't happen with our rules, but just in case
                continue;
            }

            // Try to get it as a positional argument
            let value_result = match i {
                0 => call.opt(engine_state, stack, 0),
                1 => call.opt(engine_state, stack, 1),
                _ => unreachable!(), // Our rules limit to at most 2 positional parameters
            };

            if let Ok(Some(value)) = value_result {
                let json_value = super::call_tool::convert_nu_value_to_json_value(&value, span)?;
                params.insert(param_name.to_string(), json_value);
                continue; // Skip to next parameter
            }

            // If not found as positional, try as flag (fallback)
            if let Some(value) = call.get_flag(engine_state, stack, &param_name.to_string())? {
                let json_value = super::call_tool::convert_nu_value_to_json_value(&value, span)?;
                params.insert(param_name.to_string(), json_value);
            }
        }

        // Process all parameters (including the remaining required ones) as flags
        for (param_name, _) in &prop_vec {
            // Skip parameters we've already processed as positional arguments
            if params.contains_key(param_name) {
                continue;
            }

            // Process remaining parameters as flags
            if let Some(value) = call.get_flag(engine_state, stack, &param_name.to_string())? {
                let json_value = super::call_tool::convert_nu_value_to_json_value(&value, span)?;
                params.insert(param_name.to_string(), json_value);
            }
        }
    }

    Ok(params)
}
