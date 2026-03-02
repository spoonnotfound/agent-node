use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    Process(String),
    Session(String),
    State(String),
    Timeout,
    Auth(String),
    NotFound(String),
    Config(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "io error: {}", err),
            AppError::Yaml(err) => write!(f, "yaml error: {}", err),
            AppError::Process(msg) => write!(f, "process error: {}", msg),
            AppError::Session(msg) => write!(f, "session error: {}", msg),
            AppError::State(msg) => write!(f, "state error: {}", msg),
            AppError::Timeout => write!(f, "timeout"),
            AppError::Auth(msg) => write!(f, "auth error: {}", msg),
            AppError::NotFound(msg) => write!(f, "not found: {}", msg),
            AppError::Config(msg) => write!(f, "config error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Yaml(err)
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::Process(msg)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
