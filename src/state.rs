use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Stopped,
    Running,
    Upgrading,
}

impl Default for AgentStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub env: HashMap<String, String>,
    pub status: AgentStatus,
}

impl Agent {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: "0.1.0".to_string(),
            env: HashMap::new(),
            status: AgentStatus::Stopped,
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new("default", "default")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeState {
    pub version: String,
    pub agents: HashMap<String, Agent>,
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            agents: HashMap::new(),
        }
    }
}
