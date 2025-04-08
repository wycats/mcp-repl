use std::ops::Deref;

use nu_protocol::{Record, Span, Value, ast::PathMember};

use crate::{
    mcp::McpClient,
    util::error::{McpResult, generic_error},
};

#[derive(Clone, Debug)]
pub struct ReplClient {
    pub(crate) name: String,
    pub(crate) client: McpClient,
    pub(crate) _debug: bool,
}

impl Deref for ReplClient {
    type Target = McpClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

/// Convert a JSON value to a Nushell value.
///
///
/// # Errors
///
/// This function will return an error if the JSON value cannot be converted to a Nushell value.
pub fn convert_json_value_to_nu_value(v: &serde_json::Value, span: Span) -> McpResult<Value> {
    let result = match v {
        serde_json::Value::Null => Value::Nothing {
            internal_span: span,
        },
        serde_json::Value::Bool(b) => Value::Bool {
            val: *b,
            internal_span: span,
        },
        serde_json::Value::Number(n) => {
            if let Some(val) = n.as_i64() {
                Value::Int {
                    val,
                    internal_span: span,
                }
            } else if let Some(val) = n.as_f64() {
                Value::Float {
                    val,
                    internal_span: span,
                }
            } else {
                return Err(generic_error(
                    format!("Unexpected numeric value, cannot convert {n} into i64 or f64"),
                    None,
                    None,
                ));
            }
        }
        serde_json::Value::String(val) => Value::String {
            val: val.clone(),
            internal_span: span,
        },
        serde_json::Value::Array(a) => {
            let t = a
                .iter()
                .map(|x| convert_json_value_to_nu_value(x, span))
                .collect::<McpResult<Vec<Value>>>()?;
            Value::List {
                vals: t,
                internal_span: span,
            }
        }
        serde_json::Value::Object(o) => {
            let mut cols = vec![];
            let mut vals = vec![];

            for (k, v) in o {
                cols.push(k.clone());
                vals.push(convert_json_value_to_nu_value(v, span)?);
            }

            let record = Record::from_raw_cols_vals(cols, vals, span, span).unwrap();
            Value::Record {
                val: nu_utils::SharedCow::new(record),
                internal_span: span,
            }
        }
    };

    Ok(result)
}

// Adapted from https://github.com/nushell/nushell/blob/main/crates/nu-command/src/commands/formats/to/json.rs
pub fn convert_nu_value_to_json_value(v: &Value, span: Span) -> McpResult<serde_json::Value> {
    Ok(match v {
        Value::Bool { val, .. } => serde_json::Value::Bool(*val),
        Value::Filesize { val, .. } => {
            serde_json::Value::Number(serde_json::Number::from(val.get()))
        }
        Value::Duration { val, .. } => serde_json::Value::String(val.to_string()),
        Value::Date { val, .. } => serde_json::Value::String(val.to_string()),
        Value::Float { val, .. } => {
            if let Some(num) = serde_json::Number::from_f64(*val) {
                serde_json::Value::Number(num)
            } else {
                return Err(generic_error(
                    format!("Unexpected numeric value, cannot convert {val} from f64"),
                    None,
                    None,
                ));
            }
        }
        Value::Int { val, .. } => serde_json::Value::Number(serde_json::Number::from(*val)),
        Value::Range { val, .. } => serde_json::Value::String(val.to_string()),
        Value::Glob { val, .. } | Value::String { val, .. } => {
            serde_json::Value::String(val.clone())
        }
        Value::Nothing { .. } | Value::Custom { .. } | Value::Closure { .. } => {
            serde_json::Value::Null
        }
        Value::CellPath { val, .. } => serde_json::Value::Array(
            val.members
                .iter()
                .map(|x| match &x {
                    PathMember::String { val, .. } => Ok(serde_json::Value::String(val.clone())),
                    PathMember::Int { val, .. } => Ok(serde_json::Value::Number(
                        serde_json::Number::from(*val as u64),
                    )),
                })
                .collect::<McpResult<Vec<serde_json::Value>>>()?,
        ),
        Value::List { vals, .. } => serde_json::Value::Array(json_list(vals, span)?),
        Value::Error { error, .. } => return Err(error.into()),
        Value::Binary { val, .. } => serde_json::Value::Array(
            val.iter()
                .map(|x| {
                    Ok(serde_json::Value::Number(serde_json::Number::from(
                        u64::from(*x),
                    )))
                })
                .collect::<McpResult<Vec<serde_json::Value>>>()?,
        ),
        Value::Record { val, .. } => {
            let mut m = serde_json::Map::new();
            for (k, v) in val.iter() {
                m.insert(k.clone(), convert_nu_value_to_json_value(v, span)?);
            }
            serde_json::Value::Object(m)
        }
    })
}

fn json_list(input: &[Value], span: Span) -> McpResult<Vec<serde_json::Value>> {
    let mut out = vec![];

    for value in input {
        out.push(convert_nu_value_to_json_value(value, span)?);
    }

    Ok(out)
}
