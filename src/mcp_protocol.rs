use serde::{Deserialize, Serialize};

/// MCP JSON-RPC Request
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC Response
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

/// Server Info
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Initialize Result
#[derive(Debug, Serialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: serde_json::Value,
    pub server_info: ServerInfo,
}

/// Tools List Result
#[derive(Debug, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Call Tool Result
#[derive(Debug, Serialize)]
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

impl ContentBlock {
    pub fn text(t: &str) -> Self {
        Self {
            content_type: "text".to_string(),
            text: Some(t.to_string()),
        }
    }
}

/// Call Tool Params
#[derive(Debug, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Server Capabilities
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
}
