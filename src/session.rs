use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub cwd: String,
    pub pid: u32,
    pub created_at: i64,
    pub last_active: i64,
}

impl Session {
    pub fn new(id: impl Into<String>, agent_id: impl Into<String>, cwd: impl Into<String>, pid: u32) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            cwd: cwd.into(),
            pid,
            created_at: now,
            last_active: now,
        }
    }
}

impl Session {
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
}
