use serde::{Deserialize, Serialize};

/// MCP Tool definitions for Agent Node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl McpTool {
    pub fn system_get_agents() -> Self {
        Self {
            name: "system_get_agents".to_string(),
            description: "List all registered agents".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    pub fn system_upgrade_agent() -> Self {
        Self {
            name: "system_upgrade_agent".to_string(),
            description: "Upgrade agent to a new version".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent ID" },
                    "version": { "type": "string", "description": "Target version" }
                },
                "required": ["agent_id"]
            }),
        }
    }

    pub fn system_set_env() -> Self {
        Self {
            name: "system_set_env".to_string(),
            description: "Set environment variable for agent".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent ID" },
                    "key": { "type": "string", "description": "Environment variable key" },
                    "value": { "type": "string", "description": "Environment variable value" }
                },
                "required": ["agent_id", "key", "value"]
            }),
        }
    }

    pub fn session_start() -> Self {
        Self {
            name: "session_start".to_string(),
            description: "Start a new agent session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent ID" },
                    "cwd": { "type": "string", "description": "Working directory" },
                    "command": { "type": "string", "description": "Command to run" }
                },
                "required": ["agent_id"]
            }),
        }
    }

    pub fn session_execute_turn() -> Self {
        Self {
            name: "session_execute_turn".to_string(),
            description: "Execute a turn in an existing session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID" },
                    "input": { "type": "string", "description": "Input prompt" }
                },
                "required": ["session_id", "input"]
            }),
        }
    }

    pub fn session_resolve_action() -> Self {
        Self {
            name: "session_resolve_action".to_string(),
            description: "Resolve and execute an action".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID" },
                    "action": { "type": "string", "description": "Action to resolve" }
                },
                "required": ["session_id", "action"]
            }),
        }
    }

    pub fn session_get_history() -> Self {
        Self {
            name: "session_get_history".to_string(),
            description: "Get session history".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID" },
                    "limit": { "type": "number", "description": "Max turns to return" }
                }
            }),
        }
    }

    pub fn codex_exec() -> Self {
        Self {
            name: "codex_exec".to_string(),
            description: "Execute Codex AI to perform coding tasks".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Prompt for Codex" },
                    "model": { "type": "string", "description": "Model to use (optional)" }
                },
                "required": ["prompt"]
            }),
        }
    }

    pub fn codex_update() -> Self {
        Self {
            name: "codex_update".to_string(),
            description: "Update Codex CLI to the latest version".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    pub fn codex_config() -> Self {
        Self {
            name: "codex_config".to_string(),
            description: "Get or set Codex configuration".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Config key (e.g. model, personality)" },
                    "value": { "type": "string", "description": "New value (omit to get current value)" }
                }
            }),
        }
    }

    pub fn system_get_sessions() -> Self {
        Self {
            name: "system_get_sessions".to_string(),
            description: "List all active sessions".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    pub fn system_kill_session() -> Self {
        Self {
            name: "system_kill_session".to_string(),
            description: "Kill a running session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID to kill" }
                },
                "required": ["session_id"]
            }),
        }
    }

    pub fn system_info() -> Self {
        Self {
            name: "system_info".to_string(),
            description: "Get system information (version, uptime, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

pub fn get_all_tools() -> Vec<McpTool> {
    vec![
        McpTool::system_get_agents(),
        McpTool::system_upgrade_agent(),
        McpTool::system_set_env(),
        McpTool::system_get_sessions(),
        McpTool::system_kill_session(),
        McpTool::system_info(),
        McpTool::session_start(),
        McpTool::session_execute_turn(),
        McpTool::session_resolve_action(),
        McpTool::session_get_history(),
        McpTool::codex_exec(),
        McpTool::codex_update(),
        McpTool::codex_config(),
    ]
}
