# MCP Tool Mapping

## Argument Mapping Strategy

1. If the tool has exactly one required or optional parameter, map it onto a positional argument.
2. If the tool has exactly two required parameters, map them onto positional arguments.
3. If the tool has exactly one or two required parameters and all of the rest of the arguments are optional, map the required parameters onto positional arguments and the optional parameters onto flags.
4. Optional parameters that are booleans should be mapped to switches (e.g., `--verbose`).
5. All other optional parameters should be mapped to flags (e.g., `--limit 10`).

We need to make sure that these mappings are two-way: when the tool is called, it needs to convert the arguments passed to Nushell into the correct JSON arguments for the MCP tool.

## Command Naming Conventions

1. Tools are registered under the `tool` namespace (e.g., `tool get_file_info`).
2. Keep the original tool name from MCP for consistency, but consider aliases for common operations.
3. Use the name of the MCP as part of the tool name (e.g. `fs/get_file_info`) to support a future where multiple MCPs are mounted at the same time.
4. Follow-up work: Consider grouping related tools by ensuring consistent prefixes (e.g., `get_*`, `create_*`, `update_*`).

## Parameter Handling

1. Required parameters should be clearly indicated in help text with asterisks or "(required)" labels. This should happen automatically by mapping required MCP parameters onto required Nushell parameters.
2. If the JSON schema for a parameter has a default value, that default value should be mapped onto a nushell default value.
3. For enum parameters (fixed set of choices), use a SyntaxShape::OneOf to define the valid choices.
4. Follow-up work: Consider converting camelCase parameter names from MCP to kebab-case for flags in Nushell (e.g., `maxResults` → `--max-results`).

## Type Conversions

1. Map JSON Schema types to appropriate Nushell types:

   - `string` → `string`
   - `number` → `float`
   - `integer` → `int`
   - `boolean` → `bool`
   - `array` → `list`
   - `object` → `record`

2. For complex nested objects, consider flattening where appropriate for a more shell-friendly interface.

## Response Handling

1. Convert raw responses into structured Nushell data when possible:

   - Text responses → string outputs
   - JSON responses → structured records/tables
   - Resource responses → appropriate rendering based on content type

2. For command outputs that return tables of data, ensure proper formatting with column headers.

3. Apply colorization where appropriate (e.g., errors in red, warnings in yellow).

4. For markdown content, consider rendering options:
   - Simple rendering with basic formatting preserved
   - Pass through to a markdown renderer when available
   - Convert to plain text with sensible fallbacks for formatting

## Help Documentation

1. Generate comprehensive help text from the tool schema:

   - Include a brief description of what the tool does
   - Document all parameters with types and descriptions
   - Show usage examples
   - Include any limitations or special requirements
   - This should happen by mapping the information in the MCP schema to nushell
     concepts, not by reimplementing nushell concepts manually.

2. Ensure help docs are accessible via `tool <command> --help` with proper formatting. This should happen by properly structuring the help text in nushell.

## Error Handling

1. Map MCP API errors to user-friendly Nushell error messages. We should start
   with something simple, and evolve it in the future to take advantage of
   nushell's error handling features, including diagnostics.
2. Provide context-aware error suggestions when appropriate.
3. Include detailed error information for debugging when available.
4. Ensure error messages suggest the correct syntax for the command.

## Advanced Features

1. **Tab Completion**: Implement context-aware completion for:

   - Command names
   - Parameter names
   - Enum parameter values
   - File paths for parameters that expect files

2. **Command Composition**: Ensure outputs can be piped between commands where appropriate. By ensuring that we map text, JSON and resources appropriately, it should be possible to compose commands in a natural way. For now, don't map any MCP parameters to _pipeline inputs_, but there may be useful heuristics for such a mapping that we could discover once we're using the REPL in earnest.

3. **Batch Operations**: Support processing multiple inputs via pipes for tools that accept single inputs.

4. **Custom Aliases**: Allow users to define aliases for commonly used tool commands with preset parameters. This is follow-up work, since we don't currently have any way to configure the repl.

5. **Command Discovery**: Implement a discovery mechanism to help users find relevant tools:
   - Categorize tools by function (filesystem, AI, networking, etc.)
   - Implement a search function to find tools by keyword
   - Show related commands in help output
   - This is follow-up work that is really a separate big feature around registry integration.

## Implementation Considerations

1. Use dynamic command registration to create commands at runtime based on available tools.
2. Cache tool schemas to avoid repeating schema parsing.
3. Implement proper signature generation that captures all parameter constraints.
4. Handle asynchronous operations properly to avoid blocking the shell.
5. Consider implementing progressive enhancement where complex features degrade gracefully.

## Follow-up work: Naming Conventions

1. Consider whether it's appropriate to map camelCase parameter names from MCP to kebab-case for flags in Nushell (e.g., `maxResults` → `--max-results`). It's possible that it would be preferable to stick to the MCP's raw names, at least in some cases. We should figure it out later so we can focus on the more basic mapping questions for now.
