# MCP-Nushell Integration: Dynamic Tool Support

## 1. Convert Tool Schema into Nushell Signature

- Create utility function `schema_to_signature` in `src/util/schema.rs`
  - Parse JSON schema properties into Nushell parameters
  - Map JSON types to appropriate Nushell `SyntaxShape` values
  - Handle required vs optional parameters
  - Preserve descriptions as parameter help text

- Add special handling for:
  - JSON Schema enums → Nushell autocomplete options
  - Nested objects → Nushell record shapes
  - Array types → List shapes with appropriate item types
  - Common patterns (date, email, etc.)

- Implement error handling for malformed schemas

## 2. Dynamically Generate Tool Subcommands

- Create parent `ToolCommand` in `src/commands/tool.rs`
  - Base command that will host dynamically created subcommands

- Implement dynamic command registration system:
  - Store dynamic command map in `EngineState`
  - Create utility functions to register/unregister commands at runtime

- Create registration/refresh function:
  - Fetch tools from MCP client
  - Convert each tool to a Nushell command using schema_to_signature
  - Register as subcommands of the `tool` command
  - Preserve completion support

- Add cleanup mechanism:
  - Remove old dynamic commands when tool list refreshes
  - Re-register commands with updated schemas
  - Keep engine state in sync

## 3. Tool Invocation Implementation

- Create tool invocation function in `src/commands/tool.rs`:
  - Extract arguments from Nushell call
  - Convert Nushell values to JSON for MCP
  - Validate arguments against tool schema
  - Handle default values and required parameters

- Implement core invocation logic:
  - Use MCP client to call the tool
  - Handle async execution with tokio runtime
  - Process results into Nushell values
  - Implement error handling

- Support specialized parameter handling:
  - Binary data and file paths
  - Complex nested structures

## 4. Output Formatting for Text Content

- Create content renderer in `src/util/renderer.rs`:
  - Handle different MCP content types
  - Focus on text rendering with appropriate formatting

- Implement format detection heuristics:
  - Detect markdown based on common patterns
  - Identify code snippets
  - Use metadata from MCP if available

- Text rendering options:
  - Plain text (default)
  - Markdown rendering
  - Syntax highlighting for code
  - Structured data formatting

- Design extensible system:
  - Support for future content types
  - Easy to add new renderers

## Integration and Testing Plan

1. Implementation sequence:
   - Schema to signature conversion
   - Basic tool command structure
   - Dynamic command registration
   - Tool invocation with basic output
   - Enhanced text rendering

2. Testing strategy:
   - Unit tests for schema conversion
   - Integration tests for command registration
   - End-to-end tests for tool invocation
   - Manual testing with real MCP servers
