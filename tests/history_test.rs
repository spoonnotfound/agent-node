#[cfg(test)]
mod tests {
    use agent_node::history::{HistoryStore, SessionHistory, Turn};

    #[test]
    fn test_turn_creation() {
        let turn = Turn::new("session-1", 1, "Hello", "Hi there!");
        assert_eq!(turn.session_id, "session-1");
        assert_eq!(turn.turn_id, 1);
        assert_eq!(turn.input, "Hello");
        assert_eq!(turn.output, "Hi there!");
        assert!(turn.timestamp > 0);
    }

    #[test]
    fn test_session_history_creation() {
        let session = SessionHistory::new("session-1", "agent-1", "/tmp");
        assert_eq!(session.session_id, "session-1");
        assert_eq!(session.agent_id, "agent-1");
        assert_eq!(session.cwd, "/tmp");
        assert!(session.ended_at.is_none());
        assert!(session.turns.is_empty());
    }

    #[test]
    fn test_add_turn() {
        let mut session = SessionHistory::new("session-1", "agent-1", "/tmp");
        session.add_turn("Hello", "Hi!");
        
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].input, "Hello");
        assert_eq!(session.turns[0].output, "Hi!");
    }

    #[test]
    fn test_multiple_turns() {
        let mut session = SessionHistory::new("session-1", "agent-1", "/tmp");
        session.add_turn("Hello", "Hi!");
        session.add_turn("How are you?", "Good, thanks!");
        
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].turn_id, 1);
        assert_eq!(session.turns[1].turn_id, 2);
    }

    #[test]
    fn test_session_end() {
        let mut session = SessionHistory::new("session-1", "agent-1", "/tmp");
        assert!(session.ended_at.is_none());
        
        session.end();
        
        assert!(session.ended_at.is_some());
    }

    #[test]
    fn test_history_store() {
        let mut store = HistoryStore::new();
        
        let mut session1 = SessionHistory::new("session-1", "agent-1", "/tmp");
        session1.add_turn("Hello", "Hi!");
        
        let session2 = SessionHistory::new("session-2", "agent-1", "/tmp");
        
        store.add_session(session1);
        store.add_session(session2);
        
        assert_eq!(store.sessions.len(), 2);
    }

    #[test]
    fn test_get_session() {
        let mut store = HistoryStore::new();
        let session = SessionHistory::new("session-1", "agent-1", "/tmp");
        store.add_session(session);
        
        let found = store.get_session("session-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, "session-1");
        
        let not_found = store.get_session("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_sessions() {
        let mut store = HistoryStore::new();
        
        for i in 1..=5 {
            let session = SessionHistory::new(&format!("session-{}", i), "agent-1", "/tmp");
            store.add_session(session);
        }
        
        let list = store.list_sessions(3);
        assert_eq!(list.len(), 3);
        // 应该按最新排序
        assert_eq!(list[0].session_id, "session-5");
    }

    #[test]
    fn test_serialize_history() {
        let mut store = HistoryStore::new();
        let mut session = SessionHistory::new("session-1", "agent-1", "/tmp");
        session.add_turn("Hello", "Hi!");
        session.end();
        store.add_session(session);
        
        let yaml = store.to_yaml().unwrap();
        assert!(yaml.contains("session-1"));
        assert!(yaml.contains("Hello"));
    }

    #[test]
    fn test_deserialize_history() {
        let yaml = r#"
sessions:
  - session_id: "session-1"
    agent_id: "agent-1"
    cwd: "/tmp"
    created_at: 1000
    ended_at: 2000
    turns: []
"#;
        let store = HistoryStore::from_yaml(yaml).unwrap();
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.sessions[0].session_id, "session-1");
        assert_eq!(store.sessions[0].ended_at, Some(2000));
    }
}
