#[cfg(test)]
mod tests {
    use agent_node::process::ProcessManager;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_process_manager_new() {
        let _pm = ProcessManager::new();
        assert!(true);
    }

    #[tokio::test]
    async fn test_spawn_echo_process() {
        let pm = ProcessManager::new();
        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("TEST".to_string(), "value".to_string());
        
        let handle = pm.spawn(
            "test-agent",
            "test-session",
            "echo",
            &["hello"],
            &env,
            None,
        ).await;
        
        assert!(handle.is_ok());
        
        // Wait a bit and check if process completed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_spawn_invalid_command() {
        let pm = ProcessManager::new();
        let env: HashMap<String, String> = HashMap::new();
        
        let result = pm.spawn(
            "test",
            "session",
            "nonexistent_command_xyz",
            &[],
            &env,
            None,
        ).await;
        
        // Should fail with invalid command
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_kill() {
        let pm = ProcessManager::new();
        let env: HashMap<String, String> = HashMap::new();
        
        // Spawn a long-running process (sleep)
        let handle = pm.spawn(
            "test",
            "kill-test",
            "sleep",
            &["10"],
            &env,
            None,
        ).await.unwrap();
        
        // Kill it immediately
        let kill_result = pm.kill("kill-test").await;
        assert!(kill_result.is_ok());
        
        // Should not be running anymore
        assert!(!handle.is_running().await);
    }

    #[tokio::test]
    async fn test_get_nonexistent_process() {
        let pm = ProcessManager::new();
        let result = pm.get("nonexistent").await;
        assert!(result.is_none());
    }
}
