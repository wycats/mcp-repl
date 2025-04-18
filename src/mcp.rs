use std::{borrow::Cow, sync::Arc};

use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use log::{debug, info, warn};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParam, ClientInfo, Content, Resource, ResourceTemplate, Tool},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::Value;
use tokio::process::Command;

use crate::config::McpConnectionType;

/// Client for interacting with an MCP server
#[derive(Clone, Debug)]
pub struct McpClient {
    client: Arc<RunningService<RoleClient, ClientInfo>>,
    tools: Vec<Tool>,
    _resources: Vec<Resource>,
    _templates: Vec<ResourceTemplate>,
    debug: bool,
}

impl McpClient {
    /// Create a new MCP client with the specified connection type (async version)
    pub async fn connect(connection_type: McpConnectionType, debug: bool) -> Result<Self> {
        // Initialize the MCP client based on the connection type
        let client = match connection_type {
            McpConnectionType::Sse { url } => {
                info!("Connecting via SSE: {url}");
                Self::build_sse_client(&url).await?
            }
            McpConnectionType::Command { command, env } => {
                info!("Connecting via command: {command}");
                Self::build_command_client(&command, &env.unwrap_or_default()).await?
            }
        };

        // Get server info and capabilities
        let server_info = client.peer_info();
        info!("Connected to server: {server_info:#?}");

        let server_capabilities = &server_info.capabilities;
        let has_tools = server_capabilities.tools.as_ref().is_some();
        let has_resources = server_capabilities.resources.as_ref().is_some();

        info!("Server capabilities - Tools: {has_tools}, Resources: {has_resources}");

        // Load tools if supported
        let tools = if has_tools {
            match client.list_all_tools().await {
                Ok(tools) => {
                    info!("Loaded {} tools", tools.len());
                    tools
                }
                Err(e) => {
                    warn!("Failed to load tools: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Load resources if supported
        let resources = if has_resources {
            match client.list_all_resources().await {
                Ok(resources) => {
                    info!("Loaded {} resources", resources.len());
                    resources
                }
                Err(e) => {
                    warn!("Failed to load resources: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Load resource templates if supported
        let templates = if has_resources {
            match client.list_all_resource_templates().await {
                Ok(templates) => {
                    info!("Loaded {} templates", templates.len());
                    templates
                }
                Err(e) => {
                    warn!("Failed to load templates: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Create the client instance with the loaded data
        Ok(Self {
            client: Arc::new(client),
            tools,                 // Store the tools we loaded
            _resources: resources, // Store the resources we loaded
            _templates: templates, // Store the templates we loaded
            debug,
        })
    }

    /// Build an SSE-based MCP client
    async fn build_sse_client(url: &str) -> Result<RunningService<RoleClient, ClientInfo>> {
        let transport = rmcp::transport::SseTransport::start(url)
            .await
            .context("Failed to start SSE transport")?;

        let client_info = rmcp::model::ClientInfo::default();
        let client = client_info
            .serve(transport)
            .await
            .context("Failed to initialize SSE client")?;

        Ok(client)
    }

    /// Build a command-based MCP client that launches a subprocess
    async fn build_command_client(
        cmd: &str,
        env: &IndexMap<String, String>,
    ) -> Result<RunningService<RoleClient, ClientInfo>> {
        let mut cmd_args = shell_words::split(cmd).context("Failed to parse command")?;

        // Save the command for logging before we consume parts of it
        let all_args = cmd_args.clone(); // Clone before we mutate

        let program = cmd_args.remove(0);
        let mut command = Command::new(&program);

        // Check if this is a Docker command - Docker needs special handling for interactive mode
        let is_docker = program.contains("docker")
            && all_args
                .iter()
                .any(|arg| arg == "-i" || arg == "--interactive");

        // For Docker commands, we need to pass env vars as -e KEY=VALUE arguments
        // For non-Docker commands, we use the standard envs method
        if is_docker {
            // Find Docker 'run' command position (if it exists)
            let run_pos = cmd_args.iter().position(|arg| arg == "run");

            if let Some(pos) = run_pos {
                // Add environment variables as Docker arguments after the 'run' command
                let mut docker_args = Vec::new();
                docker_args.extend_from_slice(&cmd_args[..=pos]);

                // Add each environment variable as -e KEY=VALUE
                for (key, value) in env {
                    docker_args.push("-e".to_string());
                    docker_args.push(format!("{key}={value}"));
                }

                // Add the rest of the arguments
                if pos + 1 < cmd_args.len() {
                    docker_args.extend_from_slice(&cmd_args[pos + 1..]);
                }

                command.args(docker_args);
            } else {
                // No 'run' command found, just pass arguments as is
                command.args(&cmd_args);
                // Still set environment variables on the process
                command.envs(env);
            }
        } else {
            // Non-Docker command, use standard args and envs
            command.args(&cmd_args);
            command.envs(env);
        }

        // Set up stdio with special considerations for Docker
        if is_docker {
            info!("Detected Docker in interactive mode - using special configuration");
            // For Docker in interactive mode, we need to ensure proper stdin/stdout handling
            command.stdin(std::process::Stdio::piped());
            command.stdout(std::process::Stdio::piped());
            // Allow stderr to be inherited so we can see Docker's output
            command.stderr(std::process::Stdio::inherit());
            // Don't kill the process when the parent process exits
            command.kill_on_drop(false);
        } else {
            // Standard configuration for non-Docker commands
            command.stdin(std::process::Stdio::piped());
            command.stdout(std::process::Stdio::piped());
            command.stderr(std::process::Stdio::piped());
        }

        // Log the command being executed
        info!("Starting command: {}", shell_words::join(all_args));
        debug!("Command details: {command:#?}");

        let process =
            TokioChildProcess::new(&mut command).context("Failed to start command process")?;

        let client_info = rmcp::model::ClientInfo::default();

        // Longer timeout for Docker commands
        let timeout_duration = if is_docker {
            tokio::time::Duration::from_secs(60) // Docker might need more time to pull images
        } else {
            tokio::time::Duration::from_secs(20)
        };

        info!(
            "Waiting up to {} seconds for connection to initialize...",
            timeout_duration.as_secs()
        );

        // Add a timeout for the connection
        let timeout = tokio::time::timeout(timeout_duration, client_info.serve(process))
            .await
            .context("Connection timed out")?;

        let client = timeout.context("Failed to initialize command client")?;

        Ok(client)
    }

    /// Get all available MCP tools
    #[must_use]
    pub fn get_tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Get all available MCP resources
    #[must_use]
    #[allow(clippy::used_underscore_binding, dead_code)]
    pub fn get_resources(&self) -> &[Resource] {
        &self._resources
    }

    /// Call an MCP tool with the provided parameters
    pub async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Vec<Content>> {
        // Find the tool by name
        let _tool = self
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| anyhow!("Tool not found: {}", tool_name))?;

        // Log the request if debug is enabled
        if self.debug {
            // Use Nushell formatting for the request parameters
            let nu_formatted = crate::util::format::format_json_as_nu(&params, None);

            info!("MCP REQUEST to '{tool_name}':\n{nu_formatted}");
        }

        // Call the tool with the parameters
        let result = self
            .client
            .call_tool(CallToolRequestParam {
                name: Cow::Owned(tool_name.to_string()),
                arguments: params.as_object().cloned(),
            })
            .await
            .context("Failed to call tool")?;

        // Log the response if debug is enabled
        if self.debug {
            // Use Nushell formatting for the response
            let response_value = serde_json::to_value(&result).unwrap_or_default();
            let nu_formatted = crate::util::format::format_json_as_nu(&response_value, None);

            info!("MCP RESPONSE from '{tool_name}':\n{nu_formatted}");
        }

        Ok(result.content)
    }
}
