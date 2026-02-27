#[cfg(test)]
mod tests {
    use agent_node::state::{Agent, AgentStatus, NodeState};

    #[test]
    fn test_node_state_default() {
        let state = NodeState::new();
        assert_eq!(state.version, "1.0.0");
        assert!(state.agents.is_empty());
    }

    #[test]
    fn test_agent_creation() {
        let agent = Agent::new("test-agent", "Test Agent");
        assert_eq!(agent.id, "test-agent");
        assert_eq!(agent.name, "Test Agent");
        assert_eq!(agent.version, "0.1.0");
        assert_eq!(agent.status, AgentStatus::Stopped);
    }

    #[test]
    fn test_agent_with_env() {
        let mut agent = Agent::new("test", "test");
        agent.env.insert("KEY".to_string(), "value".to_string());
        assert_eq!(agent.env.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_agent_status_transition() {
        let mut agent = Agent::new("test", "test");
        assert_eq!(agent.status, AgentStatus::Stopped);
        
        agent.status = AgentStatus::Running;
        assert_eq!(agent.status, AgentStatus::Running);
        
        agent.status = AgentStatus::Upgrading;
        assert_eq!(agent.status, AgentStatus::Upgrading);
    }

    #[test]
    fn test_serialize_node_state() {
        let state = NodeState::new();
        let yaml = serde_yaml::to_string(&state).unwrap();
        assert!(yaml.contains("version"));
        assert!(yaml.contains("agents"));
    }

    #[test]
    fn test_deserialize_node_state() {
        let yaml = r#"
version: "1.0.0"
agents: {}
"#;
        let state: NodeState = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(state.version, "1.0.0");
    }
}
