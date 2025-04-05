# Research: Virtual Subcommands in Nushell

## Overview of Nushell Command Architecture

After analyzing the Nushell codebase and its command system, we've identified that the most elegant approach to implementing dynamic tools is through a "virtual subcommand" pattern. This pattern allows a single parent command to dynamically handle subcommand dispatching without requiring explicit registration of each subcommand in the Nushell engine.

Nushell uses a command registry system where commands are registered with the engine state, and its completion system relies on this registry to provide completion suggestions. However, by implementing custom completion logic, we can create the appearance of subcommands without actually registering them.

## Current MCP-REPL Command Registration

The current MCP-REPL project registers commands through the `register_all` function in `src/commands/mod.rs`, which adds command declarations to a `StateWorkingSet` and merges the changes back into the `EngineState`. This is done during REPL initialization.

```rust
pub fn register_all(engine_state: &mut EngineState) {
    // Create a working set to register commands
    let mut working_set = StateWorkingSet::new(engine_state);

    // Register custom MCP commands
    working_set.add_decl(Box::new(ListToolsCommand {}));
    working_set.add_decl(Box::new(CallToolCommand {}));

    // Apply the changes
    let delta = working_set.render();
    if let Err(err) = engine_state.merge_delta(delta) {
        log::warn!("Error registering custom commands: {:?}", err);
    }
}
```

## Approaches to Dynamic Tool Support

We've identified two potential approaches for implementing dynamic tool support in the MCP-Nushell REPL:

### Approach 1: Virtual Subcommand Pattern

This approach uses a single parent command (`tool`) that handles all MCP tool invocations:

1. A single parent command (`ToolCommand`) that handles all MCP tool invocations
2. Custom completion logic that dynamically suggests available tools
3. Schema-driven argument parsing based on each tool's schema

With this approach, tools are invoked as: `tool <tool_name> [arguments...]`

#### Advantages

- Simple implementation
- No need to modify the engine state when tools change
- Works well even if the tool set changes frequently

#### Disadvantages

- Limited integration with Nushell's help system
- No auto-completion for tool arguments (without complex custom implementation)
- No signature-based validation of parameters
- Less consistent user experience compared to native Nushell commands

### Approach 2: Dynamic Command Registration

This approach dynamically registers each tool as a separate Nushell command, fully integrating with Nushell's command infrastructure:

1. Register each MCP tool as a dedicated Nushell command during REPL initialization
2. Convert each tool's schema into a proper Nushell signature
3. Implement re-registration when the tool set changes

With this approach, tools are invoked directly by their name: `<tool_name> [arguments...]`

#### Advantages

- Full integration with Nushell's help system and completion
- Native parameter validation and error messages
- Consistent user experience with other Nushell commands
- Tool-specific help and documentation available via `help <tool_name>`

#### Disadvantages

- More complex implementation, especially for handling dynamically changing tools
- Need to modify the engine state when tools are added or removed
- Potential performance impact when registering/unregistering many commands

## Recommended Approach: Dynamic Command Registration

Despite the additional complexity, we recommend the dynamic command registration approach as it provides the best user experience and most complete integration with Nushell's capabilities.

Here's how we'll implement it:

### 1. Tool-Specific Command Structure

Next, we need to implement the mechanism for registering and refreshing tool commands in the engine state. Similar to how we store the MCP client in the engine state, we'll need to store and track the registered tools:

```rust
// Track registered tool commands to facilitate updates
pub struct RegisteredTools {
    // Map of tool name to command ID for management
    tool_ids: HashMap<String, DeclId>,
    // The previous set of tool names for detecting changes
    tool_names: HashSet<String>,
}

impl RegisteredTools {
    pub fn new() -> Self {
        Self {
            tool_ids: HashMap::new(),
            tool_names: HashSet::new(),
        }
    }
    
    // Check if a tool needs to be registered (new) or updated (changed schema)
    pub fn needs_update(&self, tool: &Tool) -> bool {
        !self.tool_names.contains(&tool.name)
    }
    
    // Add a registered tool
    pub fn add(&mut self, tool_name: String, decl_id: DeclId) {
        self.tool_ids.insert(tool_name.clone(), decl_id);
        self.tool_names.insert(tool_name);
    }
    
    // Get tools that no longer exist and should be unregistered
    pub fn get_removed_tools(&self, current_tools: &[Tool]) -> Vec<String> {
        let current_names: HashSet<String> = current_tools.iter()
            .map(|t| t.name.clone())
            .collect();
            
        self.tool_names.difference(&current_names)
            .cloned()
            .collect()
    }
}

## Recommended Approach: Dynamic Command Registration

Despite the additional complexity, we recommend the dynamic command registration approach as it provides the best user experience and most complete integration with Nushell's capabilities.

Here's how we'll implement it:

### 1. Tool-Specific Command Structure

We'll create a specialized command structure that represents an MCP tool as a Nushell command:

```rust
#[derive(Clone)]
pub struct DynamicToolCommand {
    tool: Tool,
    state: Arc<Mutex<Option<McpClient>>>,
}

impl DynamicToolCommand {
    pub fn new(tool: Tool, state: Arc<Mutex<Option<McpClient>>>) -> Self {
        Self { tool, state }
    }
}
```

This command structure will store:

- The MCP tool metadata (from the MCP client)
- A reference to the MCP client (from the engine state)

### 2. Command Implementation

Implement the `Command` trait for this structure, converting the tool's schema into a proper Nushell signature:

```rust
impl Command for DynamicToolCommand {
    fn name(&self) -> &str {
        &self.tool.name
    }

    fn signature(&self) -> Signature {
        let mut sig = Signature::build(&self.tool.name)
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Any, Type::Any)]);

        // Add parameters based on the tool's schema
        if let Some(schema) = &self.tool.schema {
            if let Some(properties) = schema.get("properties") {
                if let Some(obj) = properties.as_object() {
                    let required = schema.get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>())
                        .unwrap_or_default();

                    for (name, prop_schema) in obj {
                        let is_required = required.contains(&name.as_str());
                        let (shape, desc, default) = parse_parameter_schema(prop_schema);

                        if is_required {
                            sig = sig.required(name, shape, &desc);
                        } else {
                            sig = sig.optional(name, shape, &desc, default.as_ref());
                        }
                    }
                }
            }
        }

        sig
    }

    fn usage(&self) -> &str {
        self.tool.description.as_deref().unwrap_or("MCP tool")
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // Extract arguments based on the signature
        let args = extract_arguments_from_call(&self.tool, call, engine_state, stack)?;

        // Create runtime for async operations
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ShellError::GenericError {
                error: "Failed to create runtime".into(),
                msg: e.to_string(),
                span: Some(call.head),
                help: None,
                inner: Vec::new(),
            })?;

        // Call the tool with the extracted arguments
        let result = runtime.block_on(async {
            let client_guard = self.state.lock().map_err(|_| {
                ShellError::GenericError {
                    error: "Failed to lock MCP client".into(),
                    msg: "Internal synchronization error".into(),
                    span: Some(call.head),
                    help: None,
                    inner: Vec::new(),
                }
            })?;

            match &*client_guard {
                Some(client) => client.call_tool(&self.tool.name, args).await,
                None => Err(ShellError::GenericError {
                    error: "MCP client not connected".into(),
                    msg: "MCP client is not initialized".into(),
                    span: Some(call.head),
                    help: Some("Connect to an MCP server first".into()),
                    inner: Vec::new(),
                }),
            }
        })?;

        // Process and return the result
        process_tool_result(result, call.head)
    }
}
```

### 3. Schema to Parameter Conversion

We'll need helper functions to convert JSON schema properties to Nushell parameter shapes:

```rust
// Parse a JSON schema property into a Nushell parameter
fn parse_parameter_schema(schema: &Value) -> (SyntaxShape, String, Option<Value>) {
    // Extract type information
    let shape = match schema.get("type").and_then(|t| t.as_str()) {
        Some("string") => SyntaxShape::String,
        Some("number") | Some("integer") => SyntaxShape::Number,
        Some("boolean") => SyntaxShape::Boolean,
        Some("array") => SyntaxShape::List(Box::new(SyntaxShape::Any)),
        Some("object") => SyntaxShape::Record(vec![]),
        _ => SyntaxShape::Any,
    };
    
    // Extract description
    let description = schema
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();
    
    // Extract default value
    let default = schema.get("default")
        .map(|v| json_value_to_nu_value(v, Span::unknown()));
    
    (shape, description, default)
}

// Convert a JSON value to a Nushell value
fn json_value_to_nu_value(value: &Value, span: Span) -> Value {
    match value {
        Value::Null => Value::Nothing { span },
        Value::Bool(b) => Value::Bool { val: *b, span },
        Value::Number(n) => {
            if n.is_i64() {
                Value::Int { val: n.as_i64().unwrap(), span }
            } else {
                Value::Float { val: n.as_f64().unwrap(), span }
            }
        },
        Value::String(s) => Value::String { val: s.clone(), span },
        Value::Array(arr) => {
            let vals = arr.iter()
                .map(|v| json_value_to_nu_value(v, span))
                .collect();
            Value::List { vals, span }
        },
        Value::Object(obj) => {
            let mut cols = Vec::new();
            let mut vals = Vec::new();
            
            for (k, v) in obj {
                cols.push(k.clone());
                vals.push(json_value_to_nu_value(v, span));
            }
            
            Value::Record { cols, vals, span }
        }
    }
}

// Extract arguments from a call based on the tool's schema
fn extract_arguments_from_call(
    tool: &Tool,
    call: &Call,
    engine_state: &EngineState,
    stack: &mut Stack,
) -> Result<serde_json::Value, ShellError> {
    let mut args = serde_json::Map::new();
    
    // Use the command signature to extract parameters
    if let Some(schema) = &tool.schema {
        if let Some(properties) = schema.get("properties") {
            if let Some(obj) = properties.as_object() {
                for (name, _) in obj {
                    // Check if this parameter was provided
                    let param_span = call.get_named_arg_span(name);
                    
                    if let Some(span) = param_span {
                        // The parameter was provided, get its value
                        let value = call.get_flag_value(engine_state, stack, name)?;
                        let json_value = nu_value_to_json_value(&value)?;
                        args.insert(name.clone(), json_value);
                    }
                }
            }
        }
    }
    
    Ok(serde_json::Value::Object(args))
}
```

This approach has several drawbacks:

- It requires modifying the engine state frequently, which can be expensive
- There's a risk of command registration/unregistration failures
- It's harder to maintain as the command registry changes
- Completions may not work reliably if commands are registered/unregistered while the user is typing

Instead, we recommend the virtual subcommand approach described next, which provides a more elegant solution.

## Recommended Approach: Virtual Subcommands

The recommended approach is to implement a single parent command that dynamically handles all tool invocations through a virtual subcommand pattern. This pattern has several advantages:

- No need to modify the engine state when tools change
- Consistent and reliable completion behavior
- Simpler implementation and maintenance
- Better performance when tool sets change frequently

### Core Implementation

```rust
pub struct ToolCommand {
    // Cache of available tools - updated periodically
    tools: Arc<Mutex<Vec<Tool>>>,
}

impl Command for ToolCommand {
    fn name(&self) -> &str { "tool" }

    fn signature(&self) -> Signature {
        Signature::build("tool")
            .optional("subcommand", SyntaxShape::String, "The tool to run")
            .rest("args", SyntaxShape::Any, "Arguments to pass to the tool")
            .category(Category::Custom("mcp".into()))
            .input_output_types(vec![(Type::Any, Type::Any)])
    }

    // Implement completion for subcommands
    fn complete(
        &self,
        ctx: &CompletionContext,
        offset: usize,
    ) -> Vec<Suggestion> {
        // If completing the first argument (subcommand)
        if offset == ctx.offset_in_call && !ctx.has_error {
            // Lock the tools cache
            if let Ok(tools) = self.tools.lock() {
                return tools
                    .iter()
                    .map(|tool| {
                        let value = tool.name.to_string();
                        let description = tool.description
                            .as_ref()
                            .map(|d| d.to_string())
                            .unwrap_or_default();

                        Suggestion {
                            value,
                            description,
                            style: None,
                            extra: None,
                        }
                    })
                    .collect();
            }
        }
        // If completing args for a specific subcommand
        else if offset > ctx.offset_in_call && !ctx.has_error {
            // Get the subcommand name
            if let Some(subcommand) = ctx.call.positional_nth(0) {
                if let Ok(subcommand) = subcommand.as_str() {
                    // Find the matching tool
                    if let Ok(tools) = self.tools.lock() {
                        if let Some(tool) = tools.iter().find(|t| t.name == subcommand) {
                            // Generate completions based on tool's schema
                            return self.complete_tool_args(tool, ctx, offset);
                        }
                    }
                }
            }
        }

        vec![]
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // Get the subcommand name
        let subcommand: Option<String> = call.opt(engine_state, stack, 0)?;

        if let Some(subcommand) = subcommand {
            // Get the MCP client
            let client = utils::get_mcp_client(engine_state)?;

            // Find the tool by name
            let tools = self.tools.lock().map_err(|_| {
                ShellError::GenericError {
                    error: "Failed to acquire lock on tools".into(),
                    msg: "Internal synchronization error".into(),
                    span: Some(call.head),
                    help: None,
                    inner: vec![],
                }
            })?;

            let tool = tools
                .iter()
                .find(|t| t.name.to_string() == subcommand)
                .ok_or_else(|| {
                    ShellError::GenericError {
                        error: format!("Unknown tool: {}", subcommand),
                        msg: "Tool not found".into(),
                        span: Some(call.head),
                        help: Some("Use 'mcp-list-tools' to see available tools".into()),
                        inner: vec![],
                    }
                })?;

            // Process arguments based on tool's schema
            let args = self.process_args_for_tool(tool, engine_state, stack, call)?;

            // Invoke the tool
            invoke_tool(client, tool, args, call.head)
        } else {
            // No subcommand specified, show help
            let mut tools_record = utils::NuValueMap::default();

            if let Ok(tools) = self.tools.lock() {
                for tool in tools.iter() {
                    let description = tool.description
                        .as_ref()
                        .map(|d| d.to_string())
                        .unwrap_or_default();

                    tools_record.add_string(&tool.name, description, call.head);
                }
            }

            Ok(PipelineData::Value(
                Value::Record {
                    val: tools_record.into_record(call.head),
                    internal_span: call.head,
                },
                None,
            ))
        }
    }
}
```

### Supporting Pattern: Custom Completion Function

The virtual subcommand pattern can be enhanced with a custom completion function that provides tool suggestions. However, this is not required as we can implement the completion logic directly in the `ToolCommand` itself.

## Detailed Implementation Strategy

### 1. Tool Registry in Engine State

Similar to how the MCP client is stored in the engine state, we'll store a registry of available tools in the engine state:

```rust
// In src/commands/utils.rs
pub fn set_tools_registry(engine: &mut EngineState, tools: Vec<Tool>) {
    let tools = Arc::new(Mutex::new(tools));
    engine.set_custom_data::<Arc<Mutex<Vec<Tool>>>>("mcp_tools", tools);
}

pub fn get_tools_registry(engine: &EngineState) -> Result<Arc<Mutex<Vec<Tool>>>, ShellError> {
    engine.get_custom_data::<Arc<Mutex<Vec<Tool>>>>("mcp_tools")
        .ok_or_else(|| ShellError::GenericError {
            error: "Tools registry not found".into(),
            msg: "MCP tools registry is not initialized".into(),
            span: None,
            help: Some("Make sure the MCP client is connected".into()),
            inner: Vec::new(),
        })
}
```

### 2. Schema to Signature Conversion

Create a utility function to convert JSON schemas to Nushell parameter definitions:

```rust
// In src/util/schema.rs
pub fn parameter_from_schema_property(
    name: &str,
    schema: &Value,
    required: bool,
) -> (String, SyntaxShape, String, Option<Value>) {
    // Extract type information
    let shape = match schema.get("type").and_then(|t| t.as_str()) {
        Some("string") => SyntaxShape::String,
        Some("number") | Some("integer") => SyntaxShape::Number,
        Some("boolean") => SyntaxShape::Boolean,
        Some("array") => SyntaxShape::List(Box::new(SyntaxShape::Any)),
        Some("object") => SyntaxShape::Record(vec![]),
        _ => SyntaxShape::Any,
    };

    // Extract description
    let description = schema
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();

    // Extract default value
    let default = schema.get("default").cloned();

    (name.to_string(), shape, description, default)
}
```

### 3. Tool Command Implementation

The core of our virtual subcommand system is the `ToolCommand` implementation:

### 4. Complete ToolCommand Implementation

Here's a more detailed implementation of the `ToolCommand`:

```rust
// In src/commands/tool.rs
use nu_protocol::
    ast::Call,
    engine::{Command, EngineState, Stack},
    Category, PipelineData, ShellError, Signature, SyntaxShape, Value, Span
};
use std::sync::{Arc, Mutex};
use crate::commands::utils;

#[derive(Clone)]
pub struct ToolCommand;

impl Command for ToolCommand {
    fn name(&self) -> &str {
        "tool"
    }

    fn signature(&self) -> Signature {
        Signature::build("tool")
            .optional(
                "tool_name",
                SyntaxShape::String,
                "The name of the MCP tool to invoke"
            )
            .rest(
                "args",
                SyntaxShape::Any,
                "Arguments to pass to the tool"
            )
            .category(Category::Custom("mcp".into()))
    }

    fn usage(&self) -> &str {
        "Invoke an MCP tool with dynamic arguments"
    }

    // Implement custom completion for both tool names and tool arguments
    fn complete(
        &self,
        ctx: &nu_protocol::engine::CommandArgs,
        offset: usize
    ) -> Vec<nu_protocol::CompletionItem> {
        // Get tools registry from engine state
        let engine_state = ctx.engine_state;
        let tools = match utils::get_tools_registry(engine_state) {
            Ok(tools) => tools,
            Err(_) => return Vec::new(), // No completions if we can't get tools
        };

        // Get current call info
        let (tool_name, rest_position) = match ctx.call.span() {
            Some(span) => {
                // Parse existing arguments to determine what we're completing
                // For simplicity, we'll just handle tool_name completion for now
                if offset == 0 && ctx.context.pos == 5 /* after 'tool ' */ {
                    // We're completing the tool name
                    (None, 0)
                } else {
                    // Try to get the tool name from args
                    match ctx.call.positional_nth(0) {
                        Some(arg) if let Ok(name) = arg.as_string() => {
                            (Some(name), 1)
                        },
                        _ => return Vec::new() // Invalid state for completion
                    }
                }
            },
            None => return Vec::new(), // Invalid state for completion
        };

        // Generate completions based on context
        match tool_name {
            None => {
                // Complete available tool names
                let tools_guard = match tools.lock() {
                    Ok(guard) => guard,
                    Err(_) => return Vec::new(),
                };

                tools_guard.iter().map(|tool| {
                    let description = tool.description
                        .as_ref()
                        .map(|d| d.to_string())
                        .unwrap_or_default();

                    nu_protocol::CompletionItem {
                        value: tool.name.to_string(),
                        description: Some(description),
                        ..Default::default()
                    }
                }).collect()
            },
            Some(name) => {
                // Complete arguments for the specific tool
                let tools_guard = match tools.lock() {
                    Ok(guard) => guard,
                    Err(_) => return Vec::new(),
                };

                // Find the specific tool
                if let Some(tool) = tools_guard.iter().find(|t| t.name.to_string() == name) {
                    // Convert schema to parameter completions
                    self.complete_tool_args(tool, ctx, offset, rest_position)
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = call.head;

        // Get the tool name from the first argument
        let tool_name: Option<String> = call.opt(engine_state, stack, 0)?;

        // Get the MCP client from engine state
        let client = utils::get_mcp_client(engine_state)?;

        if let Some(tool_name) = tool_name {
            // Get the tools registry
            let tools = utils::get_tools_registry(engine_state)?;
            let tools_guard = tools.lock().map_err(|_| {
                ShellError::GenericError {
                    error: "Failed to lock tools registry".into(),
                    msg: "Internal synchronization error".into(),
                    span: Some(span),
                    help: None,
                    inner: Vec::new(),
                }
            })?;

            // Find the tool by name
            let tool = tools_guard.iter().find(|t| t.name.to_string() == tool_name);

            if let Some(tool) = tool {
                // Build arguments based on schema
                let args = self.extract_tool_args(tool, engine_state, stack, call)?;

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

                // Execute the tool call
                let result = runtime.block_on(async {
                    client.call_tool(&tool.name, args).await
                });

                // Process the result
                match result {
                    Ok(content) => {
                        // Convert tool result to Nu value
                        process_tool_result(content, span)
                    }
                    Err(e) => Err(ShellError::GenericError {
                        error: "Tool invocation failed".into(),
                        msg: e.to_string(),
                        span: Some(span),
                        help: None,
                        inner: Vec::new(),
                    }),
                }
            } else {
                Err(ShellError::GenericError {
                    error: format!("Unknown tool: {}", tool_name),
                    msg: "The specified tool was not found".into(),
                    span: Some(span),
                    help: Some("Use 'mcp-list-tools' to see available tools".into()),
                    inner: Vec::new(),
                })
            }
        } else {
            // No tool specified, show help with available tools
            let mut results = Value::Record {
                cols: vec!["name".to_string(), "description".to_string()],
                vals: Vec::new(),
                span,
            };

            // Get available tools
            let tools = match utils::get_tools_registry(engine_state) {
                Ok(tools) => tools,
                Err(_) => {
                    return Err(ShellError::GenericError {
                        error: "No tools available".into(),
                        msg: "Tool registry not found".into(),
                        span: Some(span),
                        help: Some("Make sure the MCP client is connected".into()),
                        inner: Vec::new(),
                    });
                }
            };

            let tools_guard = tools.lock().map_err(|_| {
                ShellError::GenericError {
                    error: "Failed to lock tools registry".into(),
                    msg: "Internal synchronization error".into(),
                    span: Some(span),
                    help: None,
                    inner: Vec::new(),
                }
            })?;

            // Format tools into a record for display
            for tool in tools_guard.iter() {
                let name = Value::String {
                    val: tool.name.to_string(),
                    span,
                };

                let description = Value::String {
                    val: tool.description.as_ref()
                        .map(|d| d.to_string())
                        .unwrap_or_default(),
                    span,
                };

                results.vals.push(name);
                results.vals.push(description);
            }

            Ok(PipelineData::Value(results, None))
        }
    }
}

// Helper methods implemented separately
impl ToolCommand {
    // Extract and validate tool arguments from call
    fn extract_tool_args(
        &self,
        tool: &Tool,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
    ) -> Result<serde_json::Value, ShellError> {
        // Implementation details for parsing arguments based on schema
        // This would use the parameter_from_schema_property function
        // to understand the expected parameters
        // ...
    }

    // Generate completions for tool arguments based on schema
    fn complete_tool_args(
        &self,
        tool: &Tool,
        ctx: &nu_protocol::engine::CommandArgs,
        offset: usize,
        rest_position: usize,
    ) -> Vec<nu_protocol::CompletionItem> {
        // Generate completions based on tool schema
        // ...
        vec![]
    }
}

// Process MCP tool results into Nushell values
fn process_tool_result(
    content: Vec<rmcp::model::Content>,
    span: Span,
) -> Result<PipelineData, ShellError> {
    // Convert tool results to PipelineData
    // ...
    Ok(PipelineData::Value(Value::Nothing { span }, None))
}
```

### 5. Initializing the Tool Registry

During REPL startup and when the tool list is refreshed, we need to update the tool registry:

```rust
// In src/commands/list_tools.rs
pub fn refresh_tools(engine_state: &mut EngineState) -> Result<(), ShellError> {
    // Get the MCP client from engine state (using the existing pattern)
    let client = utils::get_mcp_client(engine_state)?;

    // Retrieve tools from the MCP client
    let tools = client.get_tools().to_vec();

    // Update the tools registry in the engine state
    utils::set_tools_registry(engine_state, tools);

    Ok(())
}
```

We can call this function:

1. During REPL initialization after connecting to an MCP server
2. When the user explicitly refreshes tools (e.g., with a `tool refresh` command)
3. Periodically if we want to automatically keep the tool list up-to-date

## Integrating with Existing Architecture

The virtual subcommand approach integrates well with the existing MCP client architecture. Much like how we're storing the MCP client in the engine state, we'll store the tool registry in the same way:

```rust
// Storage pattern matching existing MCP client pattern from memory
pub fn set_tools_registry(engine_state: &mut EngineState, tools: Vec<Tool>) {
    let tools_registry = Arc::new(Mutex::new(tools));
    engine_state.set_custom_data::<Arc<Mutex<Vec<Tool>>>>("mcp_tools", tools_registry);
}

pub fn get_tools_registry(engine_state: &EngineState) -> Result<Arc<Mutex<Vec<Tool>>>, ShellError> {
    engine_state.get_custom_data::<Arc<Mutex<Vec<Tool>>>>("mcp_tools")
        .ok_or_else(|| ShellError::GenericError {
            error: "Tools registry not found".into(),
            msg: "MCP tools registry is not initialized".into(),
            span: None,
            help: Some("Make sure the MCP client is connected".into()),
            inner: Vec::new(),
        })
}
```

This builds on the pattern already established for the MCP client storage, providing consistency in the codebase.

## Tool Registry Initialization

When the MCP tool list is refreshed, we update the tool registry in the engine state:

```rust
// In src/commands/list_tools.rs
pub fn refresh_tools(engine_state: &mut EngineState) -> Result<(), ShellError> {
    // Get the MCP client from engine state
    let client = utils::get_mcp_client(engine_state)?;

    // Retrieve tools from the MCP client
    let tools = client.get_tools().to_vec();

    // Update the tools registry
    utils::set_tools_registry(engine_state, tools);

    Ok(())
}
```

## Conclusion

The virtual subcommand approach offers the best solution for implementing dynamic tool support in the MCP-Nushell REPL. By using a single parent command with dynamic completion and argument handling, we avoid the complexity and potential issues of runtime command registration/unregistration.

Key advantages of this approach:

1. **Simplicity**: Single command implementation with virtual subcommands
2. **Reliability**: No dynamic engine state modifications for command registration
3. **Consistency**: Follows the same pattern as the MCP client storage in engine state
4. **Performance**: Minimal overhead when tool list changes
5. **Maintainability**: Clear separation between tool registry and command behavior

Implementing this pattern will provide MCP tools as first-class citizens in the Nushell environment with proper completion support. Users will be able to discover and use tools with the same experience as built-in commands, while the implementation remains clean and maintainable.
