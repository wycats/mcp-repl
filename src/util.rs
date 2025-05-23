use nu_protocol::{IntoPipelineData, PipelineData, Span, Value};
pub mod error;
pub mod format;
pub mod status;

#[derive(Clone, Debug, Default)]
pub struct NuValueMap {
    cols: Vec<String>,
    vals: Vec<Value>,
}

impl NuValueMap {
    pub fn add(&mut self, name: impl Into<String>, val: Value) {
        self.cols.push(name.into());
        self.vals.push(val);
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn add_i64(&mut self, name: impl Into<String>, val: i64, span: Span) {
        self.cols.push(name.into());
        self.vals.push(Value::int(val, span));
    }

    #[allow(dead_code)]
    pub fn add_string(&mut self, name: impl Into<String>, val: impl Into<String>, span: Span) {
        self.cols.push(name.into());
        self.vals.push(Value::string(val, span));
    }

    #[allow(dead_code)]
    pub fn add_bool(&mut self, name: impl Into<String>, val: bool, span: Span) {
        self.cols.push(name.into());
        self.vals.push(Value::bool(val, span));
    }

    #[allow(dead_code)]
    pub fn add_vec(&mut self, name: impl Into<String>, vec: Vec<Value>, span: Span) {
        self.cols.push(name.into());
        self.vals.push(Value::list(vec, span));
    }

    #[must_use]
    pub fn into_value(self, internal_span: Span) -> Value {
        // Create a record with the columns and values
        let mut record = nu_protocol::Record::new();
        for (col, val) in self.cols.iter().zip(self.vals.iter()) {
            record.insert(col.clone(), val.clone());
        }
        Value::record(record, internal_span)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn into_pipeline_data(self, span: Span) -> PipelineData {
        self.into_value(span).into_pipeline_data()
    }
}
