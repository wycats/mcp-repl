use nu_engine::CallExt;
use nu_protocol::{
    Category, IntoPipelineData, PipelineData, ShellError, Signature, Value,
    engine::{Command, EngineState, Stack},
};
use serde_json::Value as JsonValue;

use crate::engine::EngineStateExt;

use super::dynamic_commands::dynamic_tool_commands::list_tool_commands;

/// List MCP tools command
#[derive(Clone)]
pub struct ListToolsCommand;

impl Command for ListToolsCommand {
    fn name(&self) -> &str {
        "mcp-list-tools"
    }

    fn signature(&self) -> Signature {
        Signature::build(String::from("mcp-list-tools"))
            .switch("protocol", "Show protocol schema", Some('r'))
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
        let protocol = call.get_flag_span(stack, "protocol");

        list_tool_commands(engine_state, call, protocol)
    }
}
