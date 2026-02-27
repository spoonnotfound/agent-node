use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{AppError, Result};

const MAX_BUFFER_SIZE: usize = 2 * 1024 * 1024; // 2MB

#[derive(Clone)]
pub struct ProcessHandle {
    child: Arc<RwLock<Option<Child>>>,
    pub agent_id: String,
    pub session_id: String,
}

impl ProcessHandle {
    pub fn new(agent_id: impl Into<String>, session_id: impl Into<String>, child: Child) -> Self {
        Self {
            child: Arc::new(RwLock::new(Some(child))),
            agent_id: agent_id.into(),
            session_id: session_id.into(),
        }
    }

    pub async fn write_stdin(&self, input: &str) -> Result<()> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(ref mut stdin) = child.stdin {
                use std::io::Write;
                stdin.write_all(input.as_bytes())?;
                stdin.flush()?;
                return Ok(());
            }
        }
        Err(AppError::Process("stdin not available".to_string()))
    }

    pub async fn read_stdout(&self) -> Result<String> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut output = String::new();
                let mut total = 0;
                for line in reader.lines() {
                    let line = line?;
                    total += line.len();
                    if total > MAX_BUFFER_SIZE {
                        output.push_str("\n[Output truncated: exceeded 2MB limit]");
                        break;
                    }
                    output.push_str(&line);
                    output.push('\n');
                }
                return Ok(output);
            }
        }
        Ok(String::new())
    }

    pub async fn kill(&self) -> Result<()> {
        let mut child_guard = self.child.write().await;
        if let Some(mut child) = child_guard.take() {
            child.kill()?;
        }
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            return child.try_wait().map(|w| w.is_none()).unwrap_or(false);
        }
        false
    }
}

pub struct ProcessManager {
    processes: Arc<RwLock<HashMap<String, ProcessHandle>>>,
}

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
    ) -> Result<ProcessHandle> {
        let mut cmd = Command::new(command);
        cmd.args(args);

        // Environment isolation - clear all env vars and set only what we want
        cmd.env_clear();
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
        let handle = ProcessHandle::new(agent_id, session_id, child);

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

    pub async fn kill(&self, session_id: &str) -> Result<()> {
        if let Some(handle) = self.remove(session_id).await {
            handle.kill().await?;
        }
        Ok(())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}
