//! JSON-RPC 2.0 message types for the stdio transport.
//!
//! Only what the MCP stdio transport needs: one JSON object per line, requests carry an `id`,
//! notifications do not, and responses echo the `id`. Batches are not supported.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Standard JSON-RPC error codes, plus MCP's convention that an unknown tool is invalid params.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// An incoming request or notification.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub jsonrpc: Option<String>,
    /// Present (possibly `null`) for requests; absent for notifications.
    #[serde(default, deserialize_with = "deserialize_id")]
    pub id: IdField,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Distinguishes "no id key" (a notification) from `"id": null`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum IdField {
    #[default]
    Absent,
    Present(Value),
}

fn deserialize_id<'de, D>(deserializer: D) -> Result<IdField, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(IdField::Present(Value::deserialize(deserializer)?))
}

impl Request {
    /// True when the message carries no `id`, so it must never be answered.
    pub fn is_notification(&self) -> bool {
        self.id == IdField::Absent
    }

    /// The id to echo in a response (`Null` for a notification, which should not happen).
    pub fn id_value(&self) -> Value {
        match &self.id {
            IdField::Absent => Value::Null,
            IdField::Present(value) => value.clone(),
        }
    }
}

/// An error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }
}

/// Build a success response.
pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Build an error response.
pub fn failure(id: Value, error: RpcError) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

/// Parse one line. A malformed line or a batch yields the error response to send back.
pub fn parse_line(line: &str) -> Result<Request, Value> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| failure(Value::Null, RpcError::new(PARSE_ERROR, error.to_string())))?;

    if value.is_array() {
        return Err(failure(
            Value::Null,
            RpcError::new(INVALID_REQUEST, "batch requests are not supported"),
        ));
    }

    let id = value.get("id").cloned().unwrap_or(Value::Null);
    serde_json::from_value::<Request>(value).map_err(|error| {
        failure(
            id,
            RpcError::new(INVALID_REQUEST, format!("invalid request: {error}")),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_notifications_are_told_apart_by_the_id_key() {
        let request = parse_line(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).unwrap();
        assert!(!request.is_notification());
        assert_eq!(request.id_value(), json!(1));

        let null_id = parse_line(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).unwrap();
        assert!(!null_id.is_notification());

        let notification =
            parse_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert!(notification.is_notification());
        assert_eq!(notification.method, "notifications/initialized");
    }

    #[test]
    fn malformed_json_is_a_parse_error_with_null_id() {
        let response = parse_line("{not json").unwrap_err();
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], json!(PARSE_ERROR));
    }

    #[test]
    fn batches_and_missing_method_are_invalid_requests() {
        let batch = parse_line("[]").unwrap_err();
        assert_eq!(batch["error"]["code"], json!(INVALID_REQUEST));

        let no_method = parse_line(r#"{"jsonrpc":"2.0","id":"abc"}"#).unwrap_err();
        assert_eq!(no_method["error"]["code"], json!(INVALID_REQUEST));
        assert_eq!(no_method["id"], json!("abc"));
    }

    #[test]
    fn error_data_is_omitted_when_absent() {
        let plain = serde_json::to_value(RpcError::new(1, "x")).unwrap();
        assert!(plain.get("data").is_none());
        let with = serde_json::to_value(RpcError::new(1, "x").with_data(json!({"k": 1}))).unwrap();
        assert_eq!(with["data"]["k"], json!(1));
    }
}
