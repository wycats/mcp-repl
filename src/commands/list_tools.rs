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
            .category(Category::Custom(String::from("mcp")))
    }

    fn description(&self) -> &str {
        "List all available MCP tools"
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
        let client = match crate::commands::utils::get_mcp_client(engine_state) {
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

        // Get tools from the MCP client
        let result: Result<Vec<rmcp::model::Tool>, anyhow::Error> = runtime.block_on(async {
            // Use the get_tools method to fetch all tools
            Ok(client.get_tools().to_vec())
        });

        match result {
            Ok(tools) => {
                // Convert the tools to a table of records
                let mut table = Vec::new();

                for tool in tools {
                    let mut record = crate::util::NuValueMap::default();

                    record.add_string("name", tool.name.clone(), span);

                    if let Some(desc) = &tool.description {
                        record.add_string("description", desc.clone(), span);
                    }

                    // Add schema information if available
                    // input_schema is already an Arc<Map<String, Value>>, not an Option
                    record.add_string("schema", format!("{:?}", &tool.input_schema), span);

                    table.push(record.into_value(span));
                }

                Ok(PipelineData::Value(Value::list(table, span), None))
            }
            Err(e) => Err(ShellError::GenericError {
                error: "Failed to list tools".into(),
                msg: e.to_string(),
                span: Some(span),
                help: None,
                inner: Vec::new(),
            }),
        }
    }
}
