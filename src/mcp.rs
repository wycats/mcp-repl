use anyhow::{Result, anyhow};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParam, ClientInfo, Content, Resource, ResourceTemplate, Tool},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::Value;
use std::borrow::Cow;
use tokio::process::Command;

/// Type of MCP connection to establish
pub enum McpConnectionType {
    /// SSE-based MCP server (HTTP Server-Sent Events)
    Sse(String),
    /// Command-based MCP server (launches a subprocess)
    Command(String),
}

/// Client for interacting with an MCP server
pub struct McpClient {
    client: RunningService<RoleClient, ClientInfo>,
    tools: Vec<Tool>,
    resources: Vec<Resource>,
    templates: Vec<ResourceTemplate>,
}

impl McpClient {
    /// Create a new MCP client with the specified connection type
    pub async fn connect(connection_type: McpConnectionType) -> Result<Self> {
        // Initialize the MCP client based on the connection type
        let client = match connection_type {
            McpConnectionType::Sse(url) => Self::build_sse_client(&url).await?,
            McpConnectionType::Command(cmd) => Self::build_command_client(&cmd).await?,
        };

        // Create the client instance
        let mut mcp_client = Self {
            client,
            tools: Vec::new(),
            resources: Vec::new(),
            templates: Vec::new(),
        };

        // Load initial data from the server
        mcp_client.refresh_data().await?;

        Ok(mcp_client)
    }

    /// Build an SSE-based MCP client
    async fn build_sse_client(url: &str) -> Result<RunningService<RoleClient, ClientInfo>> {
        let transport = rmcp::transport::SseTransport::start(url).await?;
        let client_info = rmcp::model::ClientInfo::default();
        let client = client_info.serve(transport).await?;
        Ok(client)
    }

    /// Build a command-based MCP client that launches a subprocess
    async fn build_command_client(cmd_str: &str) -> Result<RunningService<RoleClient, ClientInfo>> {
        let mut cmd = string_to_command(cmd_str);
        let process = TokioChildProcess::new(&mut cmd)?;
        let client_info = rmcp::model::ClientInfo::default();
        let client = client_info.serve(process).await?;
        Ok(client)
    }

    /// Refresh all data from the MCP server
    pub async fn refresh_data(&mut self) -> Result<()> {
        // Refresh tools and resources
        self.refresh_tools().await?;
        self.refresh_resources().await?;

        Ok(())
    }

    /// Refresh the list of tools from the MCP server
    async fn refresh_tools(&mut self) -> Result<()> {
        // Check server capabilities
        let server_info = self.client.peer_info();
        let server_capabilities = &server_info.capabilities;

        // Only fetch tools if the server supports them
        if let Some(_) = server_capabilities.tools.as_ref() {
            self.tools = self.client.list_all_tools().await?;
        }

        Ok(())
    }

    /// Refresh the list of resources from the MCP server
    async fn refresh_resources(&mut self) -> Result<()> {
        // Check server capabilities
        let server_info = self.client.peer_info();
        let server_capabilities = &server_info.capabilities;

        // Only fetch resources if the server supports them
        if let Some(_) = server_capabilities.resources.as_ref() {
            self.resources = self.client.list_all_resources().await?;
        }

        Ok(())
    }

    /// Get server information
    pub fn server_info(&self) -> Result<String> {
        let server_info = self.client.peer_info();
        Ok(format!("{:?}", server_info))
    }

    /// Get all available MCP tools
    pub fn get_tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Get all available MCP resources
    pub fn get_resources(&self) -> &[Resource] {
        &self.resources
    }

    /// Call an MCP tool with the provided parameters
    pub async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Vec<Content>> {
        // Find the tool by name
        let _tool = self
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| anyhow!("Tool not found: {}", tool_name))?;

        // Call the tool with the parameters
        let result = self
            .client
            .call_tool(CallToolRequestParam {
                name: Cow::Owned(tool_name.to_string()),
                arguments: params.as_object().cloned(),
            })
            .await
            .map_err(|e| anyhow!("Failed to call tool: {}", e))?;

        Ok(result.content)
    }
}

/// Parse a command string into a Tokio Command
fn string_to_command(cmd_str: &str) -> Command {
    let parts: Vec<String> = shell_words::split(cmd_str)
        .unwrap_or_else(|_| vec![cmd_str.to_string()])
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut cmd = Command::new(&parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    cmd
}
