use agent_node::{NodeState, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let state = NodeState::new();
    println!("agent-node ready, version: {}", state.version);
    Ok(())
}
