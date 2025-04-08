use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Value,
    engine::{Command, EngineState, Stack},
};

use crate::engine::get_mcp_client_manager_sync;

/// List MCP resources command
#[derive(Clone)]
pub struct ListResourcesCommand;

impl Command for ListResourcesCommand {
    fn name(&self) -> &'static str {
        "resources list"
    }

    fn signature(&self) -> Signature {
        Signature::build(String::from("mcp-list-resources"))
            .category(Category::Custom(String::from("mcp")))
    }

    fn description(&self) -> &'static str {
        "List all available MCP resources"
    }

    fn run(
        &self,
        _engine_state: &EngineState,
        _stack: &mut Stack,
        call: &nu_protocol::engine::Call<'_>,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;

        let binding = get_mcp_client_manager_sync();
        let servers = binding.get_servers();

        let mut table = Vec::new();

        for (namespace, server) in servers {
            let resources = server.client.get_resources();
            for resource in resources {
                let mut record = crate::util::NuValueMap::default();

                record.add_string("uri", resource.uri.clone(), span);
                record.add_string("client", namespace.clone(), span);
                record.add_string("name", resource.name.clone(), span);

                match &resource.mime_type {
                    Some(mime) => record.add_string("type", mime.clone(), span),
                    None => record.add("type", Value::nothing(span)),
                }

                if let Some(desc) = &resource.description {
                    record.add_string("description", desc.clone(), span);
                }

                if let Some(meta) = &resource.annotations {
                    record.add_string("metadata", format!("{meta:?}"), span);
                }

                table.push(record.into_value(span));
            }
        }

        drop(binding);
        Ok(PipelineData::Value(Value::list(table, span), None))
    }
}
