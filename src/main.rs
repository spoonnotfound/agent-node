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
use tracing::{info, error, warn};

// ============ Helper Functions ============

fn get_string_arg(args: &Value, key: &str, default: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default.to_string())
}

fn get_optional_string_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}



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
        
        "codex_exec" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            if prompt.is_empty() {
                return Err("Prompt is required".to_string());
            }
            
            // Execute Codex via npx
            let result = tokio::process::Command::new("npx")
                .arg("codex")
                .arg("exec")
                .arg(prompt)
                .output()
                .await
                .map_err(|e| format!("Failed to run codex: {}", e))?;
            
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            if result.status.success() {
                Ok(json!({
                    "output": stdout,
                    "success": true
                }).to_string())
            } else {
                Ok(json!({
                    "output": stdout,
                    "error": stderr,
                    "success": false,
                    "exit_code": result.status.code()
                }).to_string())
            }
        }
        
        "codex_update" => {
            handle_codex_update().await
        }
        
        "codex_config" => {
            let key = args["key"].as_str().unwrap_or("");
            let value = args["value"].as_str();
            handle_codex_config(key, value).await
        }
        
        "system_get_sessions" => {
            let sessions = app_state.list_sessions().await;
            Ok(serde_json::to_string(&sessions).unwrap_or_default())
        }
        
        "system_kill_session" => {
            let session_id = args["session_id"].as_str().unwrap_or("");
            if session_id.is_empty() {
                return Err("session_id is required".to_string());
            }
            app_state.process_manager.kill(session_id).await
                .map_err(|e| e.to_string())?;
            app_state.remove_session(session_id).await;
            info!("Session killed: {}", session_id);
            Ok(json!({"success": true, "session_id": session_id}).to_string())
        }
        
        "system_info" => {
            let info = serde_json::json!({
                "version": "0.1.0",
                "name": "Agent Node",
                "mcp_version": "2024-11-05",
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            });
            Ok(info.to_string())
        }
        
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}

async fn handle_codex_update() -> Result<String, String> {
    // Get current version
    let current = tokio::process::Command::new("npx")
        .arg("codex")
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to check version: {}", e))?;
    
    let current_version = String::from_utf8_lossy(&current.stdout).trim().to_string();
    
    // Update Codex
    let update = tokio::process::Command::new("npm")
        .args(["install", "-g", "codex-cli@latest", "--force"])
        .output()
        .await
        .map_err(|e| format!("Failed to update: {}", e))?;
    
    let output = String::from_utf8_lossy(&update.stdout).trim().to_string();
    let error = String::from_utf8_lossy(&update.stderr).trim().to_string();
    
    // Get new version
    let new = tokio::process::Command::new("npx")
        .arg("codex")
        .arg("--version")
        .output()
        .await;
    
    let new_version = new.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|_| "unknown".to_string());
    
    Ok(json!({
        "before": current_version,
        "after": new_version,
        "output": output,
        "error": error,
        "success": update.status.success()
    }).to_string())
}

async fn handle_codex_config(key: &str, value: Option<&str>) -> Result<String, String> {
    let config_path = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".codex/config.toml");
    
    // Read current config
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    
    if key.is_empty() {
        // Return full config
        return Ok(json!({
            "config": content,
            "path": config_path.to_string_lossy()
        }).to_string());
    }
    
    // Check if setting a value
    if let Some(new_value) = value {
        // Update config
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        
        if let Some(line_num) = lines.iter().position(|l| l.starts_with(&format!("{} = ", key))) {
            lines[line_num] = format!("{} = \"{}\"", key, new_value);
        } else {
            lines.push(format!("{} = \"{}\"", key, new_value));
        }
        
        let new_content = lines.join("\n");
        std::fs::write(&config_path, &new_content)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        
        return Ok(json!({
            "key": key,
            "value": new_value,
            "success": true
        }).to_string());
    }
    
    // Get current value
    for line in content.lines() {
        if line.starts_with(&format!("{} = ", key)) {
            let val = line.split("= ").nth(1).unwrap_or("").trim_matches('"');
            return Ok(json!({
                "key": key,
                "value": val
            }).to_string());
        }
    }
    
    Err(format!("Key '{}' not found in config", key))
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

async fn call_tool(State(state): State<ServerState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let tool_name = payload["name"].as_str().unwrap_or("");
    let arguments = payload.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
    
    let result = match handle_tool_call(&state.app_state, tool_name, arguments).await {
        Ok(output) => (StatusCode::OK, Json(json!({ "success": true, "result": output }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": e }))),
    };
    result
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
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Starting Agent Node...");
    
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
        .route("/api/execute", post(call_tool))
        .layer(cors)
        .with_state(state.clone());

    // Spawn MCP stdin handler in background
    let mcp_state = state.clone();
    tokio::spawn(async move {
        handle_mcp_stdin(mcp_state).await;
    });

    // Spawn periodic persistence task (save every 30 seconds)
    let persist_state = state.app_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            if let Err(e) = persist_state.persist().await {
                error!("Failed to persist state: {}", e);
            } else {
                info!("State persisted successfully");
            }
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    println!("Agent Node MCP Server running on http://0.0.0.0:{}", port);
    println!("MCP stdio mode: echo JSON-RPC to stdin");
    println!("Set AGENT_NODE_AUTH for authentication.");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ============ Input Validation ============

fn validate_agent_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("agent_id cannot be empty".to_string());
    }
    if id.len() > 100 {
        return Err("agent_id too long (max 100 chars)".to_string());
    }
    Ok(())
}

fn validate_session_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("session_id cannot be empty".to_string());
    }
    // UUID format check - simple length check
    if id.len() != 36 || !id.contains('-') {
        return Err("invalid session_id format".to_string());
    }
    Ok(())
}

fn validate_prompt(prompt: &str) -> Result<(), String> {
    if prompt.is_empty() {
        return Err("prompt cannot be empty".to_string());
    }
    if prompt.len() > 100000 {
        return Err("prompt too long (max 100KB)".to_string());
    }
    Ok(())
}
