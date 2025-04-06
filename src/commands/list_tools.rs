use nu_engine::CallExt;
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Value,
    engine::{Command, EngineState, Stack},
};

/// List MCP tools command
#[derive(Clone)]
pub struct ListToolsCommand;

impl Command for ListToolsCommand {
    fn name(&self) -> &str {
        "mcp-list-tools"
    }

    fn signature(&self) -> Signature {
        Signature::build(String::from("mcp-list-tools"))
            .switch("long", "Show long description", Some('l'))
            .category(Category::Custom(String::from("mcp")))
    }

    fn description(&self) -> &str {
        "List all available MCP tools"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &nu_protocol::engine::Call<'_>,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;
        let long: bool = call.has_flag(engine_state, stack, "long")?;

        // Try to get the MCP client from the utils
        let client = match crate::commands::utils::get_mcp_client(engine_state) {
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

        // Get tools from the MCP client
        let tools = client.get_tools().to_vec();
        let mut record = crate::util::NuValueMap::default();

        for tool in tools {
            let mut tool_record = crate::util::NuValueMap::default();

            if let Some(desc) = &tool.description {
                tool_record.add_string("description", desc.clone(), span);
            }

            if long {
                // Add schema information if available
                // Convert the JSON schema to a proper Nu value object
                // Use schema_as_json_value() to get a serde_json::Value first
                let schema_json = tool.schema_as_json_value();
                let schema_value = match crate::commands::call_tool::convert_json_value_to_nu_value(
                    &schema_json,
                    span,
                ) {
                    Ok(value) => value,
                    Err(_) => {
                        // Fallback to string representation if conversion fails
                        Value::string(format!("{:?}", &tool.input_schema), span)
                    }
                };
                tool_record.add("schema", schema_value);
            }

            record.add(tool.name, tool_record.into_value(span));
        }

        Ok(PipelineData::Value(record.into_value(span), None))
    }
}
