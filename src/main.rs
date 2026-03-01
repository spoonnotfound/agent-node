use std::net::SocketAddr;
use std::env;
use std::io::{BufRead, BufReader};
use axum::{
    Router,
    routing::{get, post},
    extract::{State, Path, Json},
    response::IntoResponse,
    http::StatusCode,
};
use serde_json::{json, Value};
use tower_http::cors::{CorsLayer, Any};
use uuid::Uuid;
use tokio::io::AsyncWriteExt;

use agent_node::{AppState, Agent, Session, SessionHistory};
use agent_node::mcp_protocol::{
    JsonRpcRequest, JsonRpcResponse, InitializeResult, ServerCapabilities, ServerInfo,
    ToolsListResult, Tool, CallToolParams, CallToolResult, ContentBlock,
};

#[derive(Clone)]
struct ServerState {
    app_state: AppState,
    auth_token: Option<String>,
}

// ============ MCP Protocol Handler ============

async fn handle_mcp_request(state: &ServerState, request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: "2024-11-05".to_string(),
                capabilities: serde_json::json!({ "tools": {} }),
                server_info: ServerInfo {
                    name: "agent-node".to_string(),
                    version: "0.1.0".to_string(),
                },
            };
            JsonRpcResponse::success(request.id, serde_json::to_value(result).unwrap())
        }
        
        "tools/list" => {
            let tools = agent_node::get_all_tools();
            let mcp_tools: Vec<Tool> = tools.iter().map(|t| Tool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }).collect();
            let result = ToolsListResult { tools: mcp_tools };
            JsonRpcResponse::success(request.id, serde_json::to_value(result).unwrap())
        }
        
        "tools/call" => {
            let params: CallToolParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => return JsonRpcResponse::error(request.id, -32602, &format!("Invalid params: {}", e)),
            };
            
            let result = match handle_tool_call(&state.app_state, &params.name, params.arguments).await {
                Ok(output) => {
                    let content = vec![ContentBlock::text(&output)];
                    CallToolResult { content }
                }
                Err(e) => {
                    let content = vec![ContentBlock::text(&format!("Error: {}", e))];
                    CallToolResult { content }
                }
            };
            
            JsonRpcResponse::success(request.id, serde_json::to_value(result).unwrap())
        }
        
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
    }
}

async fn handle_tool_call(app_state: &AppState, tool_name: &str, args: Value) -> Result<String, String> {
    match tool_name {
        "system_get_agents" => {
            let agents = app_state.list_agents().await;
            Ok(serde_json::to_string(&agents).unwrap_or_default())
        }
        
        "system_set_env" => {
            let agent_id = args["agent_id"].as_str().unwrap_or("");
            let key = args["key"].as_str().unwrap_or("");
            let value = args["value"].as_str().unwrap_or("");
            
            match app_state.update_agent_env(agent_id, key, value).await {
                Some(_) => Ok(json!({"success": true}).to_string()),
                None => Err("Agent not found".to_string()),
            }
        }
        
        "system_upgrade_agent" => {
            let agent_id = args["agent_id"].as_str().unwrap_or("");
            let version = args["version"].as_str().unwrap_or("latest");
            
            match app_state.upgrade_agent(agent_id, version).await {
                Some(_) => Ok(json!({"success": true}).to_string()),
                None => Err("Agent not found".to_string()),
            }
        }
        
        "session_start" => {
            let agent_id = args["agent_id"].as_str().unwrap_or("default");
            let command = args["command"].as_str().unwrap_or("bash");
            let cwd = args["cwd"].as_str();
            
            let session_id = Uuid::new_v4().to_string();
            let mut spawn_env = std::collections::HashMap::new();
            
            if let Some(agent) = app_state.get_agent(agent_id).await {
                for (k, v) in agent.env {
                    spawn_env.insert(k, v);
                }
            }
            
            match app_state.process_manager.spawn(agent_id, &session_id, command, &[], &spawn_env, cwd).await {
                Ok(_handle) => {
                    let session = Session::new(&session_id, agent_id, cwd.unwrap_or("/"), 0);
                    app_state.add_session(session).await;
                    let history = SessionHistory::new(&session_id, agent_id, cwd.unwrap_or("/"));
                    app_state.add_session_history(history).await;
                    Ok(json!({"session_id": session_id, "status": "started"}).to_string())
                }
                Err(e) => Err(e.to_string()),
            }
        }
        
        "session_execute_turn" => {
            let session_id = args["session_id"].as_str().unwrap_or("");
            let input = args["input"].as_str().unwrap_or("");
            
            let handle = match app_state.process_manager.get(session_id).await {
                Some(h) => h,
                None => return Err("Session not found".to_string()),
            };
            
            if let Err(e) = handle.write_input(input).await {
                return Err(e.to_string());
            }
            
            let output = match handle.read_output().await {
                Ok(o) => o,
                Err(e) => return Err(e.to_string()),
            };
            
            if let Some(mut history) = app_state.get_session_history(session_id).await {
                history.add_turn(input, &output);
                app_state.add_session_history(history).await;
            }
            
            Ok(json!({"output": output, "turn_end": true}).to_string())
        }
        
        "session_get_history" => {
            let session_id = args["session_id"].as_str().unwrap_or("");
            match app_state.get_session_history(session_id).await {
                Some(h) => Ok(serde_json::to_string(&h).unwrap_or_default()),
                None => Err("History not found".to_string()),
            }
        }
        
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}

// ============ REST Handlers ============

async fn list_agents(State(state): State<ServerState>) -> impl IntoResponse {
    let agents = state.app_state.list_agents().await;
    (StatusCode::OK, Json(json!({ "agents": agents })))
}

async fn add_agent(State(state): State<ServerState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let id = payload["id"].as_str().unwrap_or("default");
    let name = payload["name"].as_str().unwrap_or("Agent");
    let agent = Agent::new(id, name);
    state.app_state.add_agent(agent).await;
    (StatusCode::OK, Json(json!({ "success": true, "agent_id": id })))
}

async fn upgrade_agent(State(state): State<ServerState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let agent_id = payload["agent_id"].as_str().unwrap_or("");
    let version = payload["version"].as_str().unwrap_or("latest");
    match state.app_state.upgrade_agent(agent_id, version).await {
        Some(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        None => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "agent not found" }))),
    }
}

async fn set_env(State(state): State<ServerState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let agent_id = payload["agent_id"].as_str().unwrap_or("");
    let key = payload["key"].as_str().unwrap_or("");
    let value = payload["value"].as_str().unwrap_or("");
    match state.app_state.update_agent_env(agent_id, key, value).await {
        Some(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        None => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "agent not found" }))),
    }
}

async fn create_session(State(state): State<ServerState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let agent_id = payload["agent_id"].as_str().unwrap_or("default");
    let command = payload["command"].as_str().unwrap_or("bash");
    let cwd = payload["cwd"].as_str();
    let env_map: std::collections::HashMap<String, String> = payload["env"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();
    
    let mut spawn_env = env_map;
    if let Some(agent) = state.app_state.get_agent(agent_id).await {
        for (k, v) in agent.env {
            spawn_env.insert(k, v);
        }
    }
    
    let session_id = Uuid::new_v4().to_string();
    
    match state.app_state.process_manager.spawn(agent_id, &session_id, command, &[], &spawn_env, cwd).await {
        Ok(_handle) => {
            let session = Session::new(&session_id, agent_id, cwd.unwrap_or("/"), 0);
            state.app_state.add_session(session).await;
            let history = SessionHistory::new(&session_id, agent_id, cwd.unwrap_or("/"));
            state.app_state.add_session_history(history).await;
            (StatusCode::OK, Json(json!({ "session_id": session_id, "agent_id": agent_id, "status": "started" })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_session(State(state): State<ServerState>, Path(session_id): Path<String>) -> impl IntoResponse {
    match state.app_state.get_session(&session_id).await {
        Some(session) => (StatusCode::OK, Json(json!({ "session": session }))),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "session not found" }))),
    }
}

async fn delete_session(State(state): State<ServerState>, Path(session_id): Path<String>) -> impl IntoResponse {
    let _ = state.app_state.process_manager.kill(&session_id).await;
    state.app_state.remove_session(&session_id).await;
    (StatusCode::OK, Json(json!({ "success": true, "session_id": session_id })))
}

async fn execute_turn(State(state): State<ServerState>, Path(session_id): Path<String>, Json(payload): Json<Value>) -> impl IntoResponse {
    let input = payload["input"].as_str().unwrap_or("");
    let handle = match state.app_state.process_manager.get(&session_id).await {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, Json(json!({ "error": "session not found" }))),
    };
    
    if let Err(e) = handle.write_input(input).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })));
    }
    
    let output = match handle.read_output().await {
        Ok(o) => o,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    };
    
    if let Some(mut history) = state.app_state.get_session_history(&session_id).await {
        history.add_turn(input, &output);
        state.app_state.add_session_history(history).await;
    }
    
    (StatusCode::OK, Json(json!({ "session_id": session_id, "input": input, "output": output, "turn_end": true })))
}

async fn get_history(State(state): State<ServerState>, Path(session_id): Path<String>) -> impl IntoResponse {
    match state.app_state.get_session_history(&session_id).await {
        Some(history) => (StatusCode::OK, Json(json!({ "history": history }))),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "history not found" }))),
    }
}

async fn list_histories(State(state): State<ServerState>) -> impl IntoResponse {
    let histories = state.app_state.list_histories(50).await;
    (StatusCode::OK, Json(json!({ "histories": histories })))
}

async fn list_tools() -> impl IntoResponse {
    let tools = agent_node::get_all_tools();
    (StatusCode::OK, Json(json!({ "tools": tools })))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

// ============ MCP Stdio Handler ============

async fn handle_mcp_stdin(state: ServerState) {
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin);
    
    for line in reader.lines() {
        if let Ok(line) = line {
            if line.trim().is_empty() {
                continue;
            }
            
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to parse JSON-RPC request: {}", e);
                    continue;
                }
            };
            
            let response = handle_mcp_request(&state, request).await;
            if let Ok(response_str) = serde_json::to_string(&response) {
                println!("{}", response_str);
            }
        }
    }
}

// ============ Main ============

#[tokio::main]
async fn main() {
    let port: u16 = env::var("AGENT_NODE_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);
    let auth_token = env::var("AGENT_NODE_AUTH").ok();

    let app_state = AppState::new();
    let state = ServerState { 
        app_state, 
        auth_token: auth_token.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // MCP stdio (for Claude Code integration)
        .route("/mcp", get(health))  // Placeholder for MCP
        // Health
        .route("/", get(health))
        .route("/health", get(health))
        // REST API
        .route("/api/agents", get(list_agents).post(add_agent))
        .route("/api/agents/:id/env", post(set_env))
        .route("/api/agents/:id/upgrade", post(upgrade_agent))
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/:id", get(get_session).delete(delete_session))
        .route("/api/sessions/:id/execute", post(execute_turn))
        .route("/api/histories", get(list_histories))
        .route("/api/histories/:session_id", get(get_history))
        .route("/api/tools", get(list_tools))
        .layer(cors)
        .with_state(state.clone());

    // Spawn MCP stdin handler in background
    let mcp_state = state.clone();
    tokio::spawn(async move {
        handle_mcp_stdin(mcp_state).await;
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    println!("Agent Node MCP Server running on http://0.0.0.0:{}", port);
    println!("MCP stdio mode: echo JSON-RPC to stdin");
    println!("Set AGENT_NODE_AUTH for authentication.");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
