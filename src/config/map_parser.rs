use clap::builder::TypedValueParser;
use clap::error::{Error, ErrorKind};
use indexmap::IndexMap;
use std::marker::PhantomData;

/// Custom parser for environment variables in the format KEY:VALUE
#[derive(Clone, Debug)]
pub struct EnvValueParser {
    _phantom: PhantomData<()>,
}

impl Default for EnvValueParser {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl TypedValueParser for EnvValueParser {
    type Value = IndexMap<String, String>;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, Error> {
        let s = value.to_string_lossy();

        // Parse a single KEY:VALUE pair
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Invalid key-value pair: '{}'. Expected format: 'KEY:VALUE'",
                    s
                ),
            ));
        }

        let key = parts[0].trim().to_string();
        let value = parts[1].trim().to_string();

        // Create new map for this pair
        let mut map = IndexMap::new();
        map.insert(key, value);

        Ok(map)
    }
}

/// Creates a new environment variable parser
///
/// This parser handles values in the format KEY:VALUE and supports multiple
/// occurrences of the same argument flag, combining them into a single map.
pub fn parse_env() -> EnvValueParser {
    EnvValueParser::default()
}
