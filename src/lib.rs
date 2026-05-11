mod parser;
mod serializer;

pub use parser::from_str;
pub use serializer::to_string;

/// PopLine DOM value type.
#[derive(Debug, Clone, PartialEq)]
pub enum PlnValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Object(Vec<(String, PlnValue)>),
    Array(Vec<PlnValue>),
}

impl PlnValue {
    pub fn new_object() -> Self { PlnValue::Object(Vec::new()) }
    pub fn new_array() -> Self { PlnValue::Array(Vec::new()) }
    pub fn new_null() -> Self { PlnValue::Null }
    pub fn new_bool(v: bool) -> Self { PlnValue::Bool(v) }
    pub fn new_int(v: i64) -> Self { PlnValue::Int(v) }
    pub fn new_float(v: f64) -> Self { PlnValue::Float(v) }
    pub fn new_string(s: &str) -> Self { PlnValue::String(s.to_string()) }
}
