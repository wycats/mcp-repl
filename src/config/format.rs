use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use config::{Config, Environment, File, FileFormat, FileSourceFile, FileSourceString};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::parse_env;
use crate::{CliArgs, commands::utils::ReplClient, mcp::McpClient};

// Define an enum that encapsulates the different possible config sources
#[derive(Debug)]
pub enum ConfigSource {
    FilePath(File<FileSourceFile, FileFormat>),
    #[allow(dead_code)]
    FileContent(File<FileSourceString, FileFormat>),
}

impl McpConnectionType {
    pub async fn to_client(&self, name: &str) -> Result<Arc<ReplClient>> {
        let client = McpClient::connect(self.clone(), false).await?;
        Ok(Arc::new(ReplClient {
            name: name.to_string(),
            client,
            _debug: false,
        }))
    }
}

/// Type of MCP connection to establish
#[derive(Clone, Debug, Deserialize, Serialize, clap::Parser)]
#[serde(untagged)]
pub enum McpConnectionType {
    /// SSE-based MCP server (HTTP Server-Sent Events)
    Sse { url: String },
    /// Command-based MCP server (launches a subprocess)
    Command {
        command: String,
        #[arg(value_parser = parse_env(), long, action = clap::ArgAction::Append)]
        env: Option<IndexMap<String, String>>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpReplConfig {
    /// List of configured MCP servers
    #[serde(default)]
    pub servers: IndexMap<String, McpConnectionType>,
}

impl Default for McpReplConfig {
    fn default() -> Self {
        Self {
            servers: IndexMap::new(),
        }
    }
}

pub trait McpConfigLoader {
    fn load_raw_env(&self) -> IndexMap<String, String>;

    fn load_env(&self) -> Environment {
        let env = self.load_raw_env();
        Environment::with_prefix("MCP")
            .separator("_")
            .source(Some(env))
    }

    fn load_env_config(&self) -> Result<Option<ConfigSource>> {
        let env = self.load_raw_env();
        env.get("MCP_CONFIG").map_or_else(
            || Ok(None),
            |path| {
                let path = PathBuf::from(path);
                self.load_file(Some(path))
            },
        )
    }

    /// Return a `ConfigSource` enum to clearly define the possible source types
    fn load_system_config(&self) -> Result<Option<ConfigSource>>;
    fn load_user_config(&self) -> Result<Option<ConfigSource>>;
    fn load_local_config(&self) -> Result<Option<ConfigSource>>;
    fn load_file(&self, path: Option<PathBuf>) -> Result<Option<ConfigSource>>;
}

#[derive(Debug, Clone)]
struct DiskConfigLoader;

impl McpConfigLoader for DiskConfigLoader {
    fn load_raw_env(&self) -> IndexMap<String, String> {
        // Since we're deserializing into an IndexMap, it can't fail.
        envy::from_env().unwrap()
    }

    fn load_file(&self, path: Option<PathBuf>) -> Result<Option<ConfigSource>> {
        match path {
            Some(path) if path.exists() => Ok(Some(ConfigSource::FilePath(
                File::from(path).required(false),
            ))),
            _ => Ok(None),
        }
    }

    fn load_system_config(&self) -> Result<Option<ConfigSource>> {
        let path = system_config_path();
        if path.exists() {
            Ok(Some(ConfigSource::FilePath(
                File::from(path).required(false),
            )))
        } else {
            Ok(None)
        }
    }

    fn load_user_config(&self) -> Result<Option<ConfigSource>> {
        let path = user_config_path();
        self.load_file(path)
    }

    fn load_local_config(&self) -> Result<Option<ConfigSource>> {
        let path = PathBuf::from("./mcp-repl.toml");
        self.load_file(Some(path))
    }
}

impl McpReplConfig {
    pub fn env(config: &CliArgs) -> Result<Self> {
        Self::load(&DiskConfigLoader, config)
    }

    /// Load configuration from the default paths
    pub fn load(loader: &dyn McpConfigLoader, config: &CliArgs) -> Result<Self> {
        // Try to load from several places, in order of preference:
        // 1. $MCP_CONFIG if specified
        // 2. ./mcp-repl.toml in current directory
        // 3. ~/.config/mcp-repl/config.toml
        // 4. /etc/mcp-repl/config.toml

        let mut builder = Config::builder();

        builder = builder.add_source(config.clone());

        // Add default config
        builder = builder.add_source(config::File::from_str(
            include_str!("../config/data/default.toml"),
            FileFormat::Toml,
        ));

        builder = add_config_source(builder, loader.load_system_config()?);
        builder = add_config_source(builder, loader.load_user_config()?);
        builder = add_config_source(builder, loader.load_local_config()?);
        builder = add_config_source(builder, loader.load_env_config()?);

        // Environment variable overrides
        builder = builder.add_source(loader.load_env());

        // Build the config
        let result = match builder.build() {
            Ok(config) => {
                log::debug!("{config:#?}");
                Ok(config.try_deserialize()?)
            }
            Err(e) => return Err(anyhow::anyhow!("Config error: {}", e)),
        };
        log::debug!("result: {result:#?}");
        result
    }
}

// Helper function to add a config source to the builder
fn add_config_source(
    builder: config::ConfigBuilder<config::builder::DefaultState>,
    source: Option<ConfigSource>,
) -> config::ConfigBuilder<config::builder::DefaultState> {
    match source {
        Some(ConfigSource::FilePath(file)) => builder.add_source(file),
        Some(ConfigSource::FileContent(file)) => builder.add_source(file),
        None => builder,
    }
}

fn system_config_path() -> PathBuf {
    PathBuf::from("/etc/mcp-repl/config.toml")
}

fn user_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("mcp-repl").join("config.toml"))
}

#[cfg(test)]
/// We're not going to use the real environment for testing, but rather
/// create a test configuration loader that simulates files and environment
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct TestConfigLoader {
        env: IndexMap<String, String>,
        configs: HashMap<String, String>, // path -> content
    }

    impl TestConfigLoader {
        fn new() -> Self {
            Self {
                env: IndexMap::new(),
                configs: HashMap::new(),
            }
        }

        fn with_env(mut self, key: &str, value: &str) -> Self {
            self.env.insert(key.to_string(), value.to_string());
            self
        }

        fn with_config(mut self, path: &str, content: &str) -> Self {
            self.configs.insert(path.to_string(), content.to_string());
            self
        }
    }

    impl McpConfigLoader for TestConfigLoader {
        fn load_raw_env(&self) -> IndexMap<String, String> {
            self.env.clone()
        }

        fn load_system_config(&self) -> Result<Option<ConfigSource>> {
            if let Some(content) = self.configs.get("/etc/mcp-repl/config.toml") {
                Ok(Some(ConfigSource::FileContent(File::from_str(
                    content,
                    FileFormat::Toml,
                ))))
            } else {
                Ok(None)
            }
        }

        fn load_user_config(&self) -> Result<Option<ConfigSource>> {
            if let Some(content) = self.configs.get("~/.config/mcp-repl/config.toml") {
                Ok(Some(ConfigSource::FileContent(File::from_str(
                    content,
                    FileFormat::Toml,
                ))))
            } else {
                Ok(None)
            }
        }

        fn load_local_config(&self) -> Result<Option<ConfigSource>> {
            if let Some(content) = self.configs.get("./mcp-repl.toml") {
                Ok(Some(ConfigSource::FileContent(File::from_str(
                    content,
                    FileFormat::Toml,
                ))))
            } else {
                Ok(None)
            }
        }

        fn load_env_config(&self) -> Result<Option<ConfigSource>> {
            if let Some(config_path) = self.env.get("MCP_CONFIG") {
                if let Some(content) = self.configs.get(config_path) {
                    Ok(Some(ConfigSource::FileContent(File::from_str(
                        content,
                        FileFormat::Toml,
                    ))))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }

        fn load_file(&self, path: Option<PathBuf>) -> Result<Option<ConfigSource>> {
            if let Some(path) = path {
                if let Some(path_str) = path.to_str() {
                    if let Some(content) = self.configs.get(path_str) {
                        return Ok(Some(ConfigSource::FileContent(File::from_str(
                            content,
                            FileFormat::Toml,
                        ))));
                    }
                }
            }
            Ok(None)
        }
    }

    #[test]
    fn test_env() {
        // Test with empty config
        let loader = TestConfigLoader::new();
        let config = McpReplConfig::load(&loader, &CliArgs::default()).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_with_mocked_config() {
        let loader = TestConfigLoader::new()
            .with_env("MCP_DEFAULT_SERVER", "test-server")
            .with_config(
                "./mcp-repl.toml",
                r#"
                [[servers]]
                name = "test-server"
                connection_type = { Sse = { url = "http://localhost:8080" } }
            "#,
            );

        let config = McpReplConfig::load(&loader, &CliArgs::default()).unwrap();

        assert!(config.find_server("test-server").is_some());
    }
}
