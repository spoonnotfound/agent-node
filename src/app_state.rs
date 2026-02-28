use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::state::Agent;
use crate::history::HistoryStore;

#[derive(Clone)]
pub struct AppState {
    pub agents: Arc<RwLock<HashMap<String, Agent>>>,
    pub history: Arc<RwLock<HistoryStore>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HistoryStore::new())),
        }
    }

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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
