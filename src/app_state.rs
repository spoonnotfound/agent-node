use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::BufReader;
use std::collections::HashMap;

use crate::state::Agent;
use crate::history::{HistoryStore, SessionHistory};
use crate::session::Session;

#[derive(Clone)]
pub struct AppState {
    pub agents: Arc<RwLock<HashMap<String, Agent>>>,
    pub history: Arc<RwLock<HistoryStore>>,
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub process_manager: Arc<ProcessManager>,
}

pub struct ProcessManager {
    processes: Arc<RwLock<HashMap<String, ProcessHandle>>>,
}

#[derive(Clone)]
pub struct ProcessHandle {
    pub agent_id: String,
    pub session_id: String,
    pub pid: u32,
    child: Arc<RwLock<Option<tokio::process::Child>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HistoryStore::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            process_manager: Arc::new(ProcessManager::new()),
        }
    }

    // Agent methods
    pub async fn add_agent(&self, agent: Agent) {
        let mut agents = self.agents.write().await;
        agents.insert(agent.id.clone(), agent);
    }

    pub async fn get_agent(&self, id: &str) -> Option<Agent> {
        let agents = self.agents.read().await;
        agents.get(id).cloned()
    }

    pub async fn list_agents(&self) -> Vec<Agent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    pub async fn update_agent_env(&self, id: &str, key: &str, value: &str) -> Option<()> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(id) {
            agent.env.insert(key.to_string(), value.to_string());
            return Some(());
        }
        None
    }

    pub async fn upgrade_agent(&self, id: &str, version: &str) -> Option<()> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(id) {
            agent.version = version.to_string();
            return Some(());
        }
        None
    }

    // Session methods
    pub async fn add_session(&self, session: Session) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }

    pub async fn get_session(&self, id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    pub async fn list_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    pub async fn remove_session(&self, id: &str) -> Option<Session> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id)
    }

    // History methods
    pub async fn add_session_history(&self, history: SessionHistory) {
        let mut store = self.history.write().await;
        store.add_session(history);
    }

    pub async fn get_session_history(&self, session_id: &str) -> Option<SessionHistory> {
        let store = self.history.read().await;
        store.get_session(session_id).cloned()
    }

    pub async fn list_histories(&self, limit: usize) -> Vec<SessionHistory> {
        let store = self.history.read().await;
        store.list_sessions(limit).into_iter().cloned().collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ProcessManager implementation
impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn spawn(
        &self,
        agent_id: &str,
        session_id: &str,
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<ProcessHandle, crate::error::AppError> {
        use std::process::Stdio;
        use tokio::process::Command;
        
        let mut cmd = Command::new(command);
        cmd.args(args);

        cmd.env_clear();
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        for (key, value) in env {
            cmd.env(key, value);
        }

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn()?;
        let handle = ProcessHandle::new(agent_id.to_string(), session_id.to_string(), child);

        let mut processes = self.processes.write().await;
        processes.insert(session_id.to_string(), handle.clone());

        Ok(handle)
    }

    pub async fn get(&self, session_id: &str) -> Option<ProcessHandle> {
        let processes = self.processes.read().await;
        processes.get(session_id).cloned()
    }

    pub async fn remove(&self, session_id: &str) -> Option<ProcessHandle> {
        let mut processes = self.processes.write().await;
        processes.remove(session_id)
    }

    pub async fn kill(&self, session_id: &str) -> Result<(), crate::error::AppError> {
        if let Some(handle) = self.remove(session_id).await {
            handle.kill().await?;
        }
        Ok(())
    }
}

// ProcessHandle implementation
impl ProcessHandle {
    pub fn new(agent_id: String, session_id: String, child: tokio::process::Child) -> Self {
        let pid = child.id().unwrap_or(0);
        Self {
            agent_id,
            session_id,
            pid,
            child: Arc::new(RwLock::new(Some(child))),
        }
    }

    pub async fn write_input(&self, input: &str) -> Result<(), crate::error::AppError> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(ref mut stdin) = child.stdin {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(input.as_bytes()).await?;
                stdin.flush().await?;
                return Ok(());
            }
        }
        Err(crate::error::AppError::Process("stdin not available".to_string()))
    }

    pub async fn read_output(&self) -> Result<String, crate::error::AppError> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(stdout) = child.stdout.take() {
                use tokio::io::AsyncBufReadExt;
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                let mut output = String::new();
                let mut total = 0;
                
                while let Ok(Some(line)) = lines.next_line().await {
                    total += line.len();
                    if total > 2 * 1024 * 1024 {
                        output.push_str("\n[Output truncated: exceeded 2MB limit]");
                        break;
                    }
                    output.push_str(&line);
                    output.push('\n');
                    
                    if line.contains("TURN_END") || line.contains("回合结束") {
                        break;
                    }
                }
                return Ok(output);
            }
        }
        Ok(String::new())
    }

    pub async fn kill(&self) -> Result<(), crate::error::AppError> {
        let mut child_guard = self.child.write().await;
        if let Some(mut child) = child_guard.take() {
            child.kill().await?;
        }
        Ok(())
    }
}
