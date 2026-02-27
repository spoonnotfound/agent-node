use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub session_id: String,
    pub turn_id: u32,
    pub input: String,
    pub output: String,
    pub timestamp: i64,
}

impl Turn {
    pub fn new(session_id: impl Into<String>, turn_id: u32, input: impl Into<String>, output: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Self {
            session_id: session_id.into(),
            turn_id,
            input: input.into(),
            output: output.into(),
            timestamp: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionHistory {
    pub session_id: String,
    pub agent_id: String,
    pub cwd: String,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub turns: Vec<Turn>,
}

impl SessionHistory {
    pub fn new(session_id: impl Into<String>, agent_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Self {
            session_id: session_id.into(),
            agent_id: agent_id.into(),
            cwd: cwd.into(),
            created_at: now,
            ended_at: None,
            turns: Vec::new(),
        }
    }

    pub fn add_turn(&mut self, input: impl Into<String>, output: impl Into<String>) {
        let turn_id = self.turns.len() as u32 + 1;
        self.turns.push(Turn::new(&self.session_id, turn_id, input, output));
    }

    pub fn end(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.ended_at = Some(now);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryStore {
    pub sessions: Vec<SessionHistory>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    pub fn add_session(&mut self, session: SessionHistory) {
        self.sessions.push(session);
    }

    pub fn get_session(&self, session_id: &str) -> Option<&SessionHistory> {
        self.sessions.iter().find(|s| s.session_id == session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut SessionHistory> {
        self.sessions.iter_mut().find(|s| s.session_id == session_id)
    }

    pub fn list_sessions(&self, limit: usize) -> Vec<&SessionHistory> {
        self.sessions.iter().rev().take(limit).collect()
    }

    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}
