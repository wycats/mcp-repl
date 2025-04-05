use nu_protocol::{
    Category, PipelineData, ShellError, Signature, Value,
    engine::{Command, EngineState, Stack},
};

/// List MCP resources command
#[derive(Clone)]
pub struct ListResourcesCommand;

impl Command for ListResourcesCommand {
    fn name(&self) -> &str {
        "mcp-list-resources"
    }

    fn signature(&self) -> Signature {
        Signature::build(String::from("mcp-list-resources"))
            .category(Category::Custom(String::from("mcp")))
    }

    fn description(&self) -> &str {
        "List all available MCP resources"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        _stack: &mut Stack,
        call: &nu_protocol::engine::Call<'_>,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;

        // Try to get the MCP client from the utils
        let client = match super::utils::get_mcp_client(engine_state) {
            Ok(client) => client,
            Err(err) => {
                return Err(ShellError::GenericError {
                    error: "Could not access MCP client".into(),
                    msg: err.into(),
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

        // Get resources from the MCP client
        let result: Result<Vec<rmcp::model::Resource>, anyhow::Error> = runtime.block_on(async {
            // Use the get_resources method to fetch all resources
            Ok(client.get_resources().to_vec())
        });

        match result {
            Ok(resources) => {
                // Convert the resources to a table of records
                let mut table = Vec::new();

                for resource in resources {
                    let mut record = crate::util::NuValueMap::default();

                    record.add_string("name", resource.name.clone(), span);
                    record.add_string("uri", resource.uri.clone(), span);

                    match &resource.mime_type {
                        Some(mime) => record.add_string("type", mime.clone(), span),
                        None => record.add("type", Value::nothing(span)),
                    };

                    if let Some(desc) = &resource.description {
                        record.add_string("description", desc.clone(), span);
                    }

                    if let Some(meta) = &resource.annotations {
                        record.add_string("metadata", format!("{:?}", meta), span);
                    }

                    table.push(record.into_value(span));
                }

                Ok(PipelineData::Value(Value::list(table, span), None))
            }
            Err(e) => Err(ShellError::GenericError {
                error: "Failed to list resources".into(),
                msg: e.to_string(),
                span: Some(span),
                help: None,
                inner: Vec::new(),
            }),
        }
    }
}
