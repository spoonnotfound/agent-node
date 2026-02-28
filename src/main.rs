use std::net::SocketAddr;
use axum::{
    Router,
    routing::{get, post},
    extract::State,
    Json,
    response::IntoResponse,
};
use serde_json::{json, Value};

use agent_node::{
    AppState, Agent,
    Result,
};

#[derive(Clone)]
struct ServerState {
    app_state: AppState,
}

async fn list_agents(State(state): State<ServerState>) -> impl IntoResponse {
    let agents = state.app_state.list_agents().await;
    Json(json!({
        "agents": agents
    }))
}

async fn add_agent(
    State(state): State<ServerState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let id = payload["id"].as_str().unwrap_or("default");
    let name = payload["name"].as_str().unwrap_or("Agent");
    
    let agent = Agent::new(id, name);
    state.app_state.add_agent(agent).await;
    
    Json(json!({
        "success": true,
        "agent_id": id
    }))
}

async fn set_env(
    State(state): State<ServerState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let agent_id = payload["agent_id"].as_str().unwrap_or("");
    let key = payload["key"].as_str().unwrap_or("");
    let value = payload["value"].as_str().unwrap_or("");
    
    match state.app_state.update_agent_env(agent_id, key, value).await {
        Some(_) => Json(json!({"success": true})),
        None => Json(json!({"success": false, "error": "agent not found"})),
    }
}

async fn list_tools() -> impl IntoResponse {
    let tools = agent_node::get_all_tools();
    Json(json!({
        "tools": tools
    }))
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

#[tokio::main]
async fn main() -> Result<()> {
    let app_state = AppState::new();
    let state = ServerState { app_state };

    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/api/agents", get(list_agents).post(add_agent))
        .route("/api/agents/:id/env", post(set_env))
        .route("/api/tools", get(list_tools))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Agent Node server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
