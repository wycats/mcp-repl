use anyhow::Result;
use nu_engine::CallExt;
use nu_protocol::{
    Category, IntoPipelineData, PipelineData, ShellError, Signature, SyntaxShape, Type, Value,
    engine::{Call, EngineState, Stack},
};

use crate::commands::tool::register_dynamic_tool;

/// Register test dynamic commands for development purposes
pub fn register_test_commands(engine_state: &mut EngineState) -> Result<()> {
    // Register a simple hello world command that demonstrates dynamic registration
    register_hello_command(engine_state)?;

    // Register a simple echo command that demonstrates argument handling
    register_echo_command(engine_state)?;

    Ok(())
}

/// Register a simple hello command that demonstrates dynamic command registration
fn register_hello_command(engine_state: &mut EngineState) -> Result<()> {
    // Create a signature for the command - this is created at runtime
    let signature = Signature::build("tool hello")
        .required("name", SyntaxShape::String, "Name to greet")
        .category(Category::Custom("test".into()))
        .input_output_types(vec![(Type::Nothing, Type::String)]);

    // Create a run function for the command - this is created at runtime
    let run_fn = Box::new(
        |engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData| {
            // Get the name argument
            let name: Option<String> = call.opt(engine_state, stack, 0)?;

            if let Some(name) = name {
                // Create a greeting
                let greeting = format!("Hello, {}!", name);
                Ok(Value::string(greeting, call.head).into_pipeline_data())
            } else {
                Err(ShellError::MissingParameter {
                    param_name: "name".into(),
                    span: call.head,
                })
            }
        },
    );

    // Register the command - this is truly dynamic as it's registered at runtime
    // with just a signature and function, not a static Command implementation
    register_dynamic_tool(
        engine_state,
        "tool hello",
        signature,
        "Say hello to someone".into(),
        run_fn,
    )
}

/// Register a simple echo command that demonstrates argument handling
fn register_echo_command(engine_state: &mut EngineState) -> Result<()> {
    // Create a signature for the command - created at runtime
    let signature = Signature::build("tool echo")
        .rest("words", SyntaxShape::String, "Words to echo back")
        .switch("uppercase", "Convert output to uppercase", Some('u'))
        .switch("reverse", "Reverse the output", Some('r'))
        .category(Category::Custom("test".into()))
        .input_output_types(vec![(Type::Nothing, Type::String)]);

    // Create a run function for the command - created at runtime
    let run_fn = Box::new(
        |engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData| {
            // Get all rest arguments as strings
            let mut words: Vec<String> = Vec::new();
            let mut i = 0;
            loop {
                match call.opt::<String>(engine_state, stack, i) {
                    Ok(Some(word)) => words.push(word),
                    Ok(None) => break, // No more arguments
                    Err(_) => break,   // Error getting argument
                }
                i += 1;
            }

            // Join words with spaces
            let mut result = words.join(" ");

            // Check for --uppercase flag
            if let Ok(Some(true)) = call.get_flag(engine_state, stack, "uppercase") {
                result = result.to_uppercase();
            }

            // Check for --reverse switch
            if call.has_flag(engine_state, stack, "reverse")? {
                result = result.chars().rev().collect();
            }

            // Return the result
            Ok(Value::string(result, call.head).into_pipeline_data())
        },
    );

    // Register the command - truly dynamic with runtime signature and function
    register_dynamic_tool(
        engine_state,
        "tool echo",
        signature,
        "Echo text with optional transformations".into(),
        run_fn,
    )
}
