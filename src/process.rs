use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::error::{AppError, Result};

const MAX_BUFFER_SIZE: usize = 2 * 1024 * 1024; // 2MB

#[derive(Clone)]
pub struct ProcessHandle {
    pub agent_id: String,
    pub session_id: String,
    pub pid: u32,
    child: Arc<RwLock<Option<tokio::process::Child>>>,
}

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

    pub async fn write_input(&self, input: &str) -> Result<()> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(ref mut stdin) = child.stdin {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(input.as_bytes()).await?;
                stdin.flush().await?;
                return Ok(());
            }
        }
        Err(AppError::Process("stdin not available".to_string()))
    }

    pub async fn read_output(&self) -> Result<String> {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                let mut output = String::new();
                let mut total = 0;
                
                while let Ok(Some(line)) = lines.next_line().await {
                    total += line.len();
                    if total > MAX_BUFFER_SIZE {
                        output.push_str("\n[Output truncated: exceeded 2MB limit]");
                        break;
                    }
                    output.push_str(&line);
                    output.push('\n');
                    
                    // Check for turn-end marker
                    if line.contains("TURN_END") || line.contains("回合结束") {
                        break;
                    }
                }
                return Ok(output);
            }
        }
        Ok(String::new())
    }

    pub async fn kill(&self) -> Result<()> {
        let mut child_guard = self.child.write().await;
        if let Some(mut child) = child_guard.take() {
            child.kill().await?;
        }
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        let mut child_guard = self.child.write().await;
        if let Some(ref mut child) = *child_guard {
            return match child.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) => true,
                Err(_) => false,
            };
        }
        false
    }
}

#[derive(Clone)]
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

        // Environment isolation
        cmd.env_clear();
        // Add default PATH
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
