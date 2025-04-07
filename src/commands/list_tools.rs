use nu_engine::CallExt;
use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Value,
    engine::{Command, EngineState, Stack},
};

use crate::engine::EngineStateExt;

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

        let clients = engine_state.get_mcp_client_manager().get_clients();

        let mut record = crate::util::NuValueMap::default();
        for (name, client) in clients.iter() {
            let tools = client.get_tools().to_vec();

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
                    let schema_value =
                        match crate::commands::call_tool::convert_json_value_to_nu_value(
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

                record.add(name, tool_record.into_value(span));
            }
        }
        Ok(PipelineData::Value(record.into_value(span), None))
    }
}
