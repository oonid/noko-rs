use std::env;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    Missing(String),
    #[error("invalid value for {0}: {1}")]
    Invalid(String, String),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub auth_mode: String,
    pub nocodb_service_token: Option<String>,
    pub nocodb_service_actor_id: Option<Uuid>,
    pub db_tx_max_retries: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::Missing("DATABASE_URL".to_string()))?;

        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let auth_mode = env::var("AUTH_MODE").unwrap_or_else(|_| "dev".to_string());
        let nocodb_service_token = env::var("NOCODB_SERVICE_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());

        let nocodb_service_actor_id = match env::var("NOCODB_SERVICE_ACTOR_ID") {
            Ok(val) if !val.is_empty() => {
                let uuid = Uuid::parse_str(&val).map_err(|e| {
                    ConfigError::Invalid("NOCODB_SERVICE_ACTOR_ID".to_string(), e.to_string())
                })?;
                Some(uuid)
            }
            _ => None,
        };

        let db_tx_max_retries = match env::var("DB_TX_MAX_RETRIES") {
            Ok(val) if !val.is_empty() => val.parse::<u32>().map_err(|e| {
                ConfigError::Invalid("DB_TX_MAX_RETRIES".to_string(), e.to_string())
            })?,
            _ => 3,
        };

        Ok(Self {
            database_url,
            bind_addr,
            auth_mode,
            nocodb_service_token,
            nocodb_service_actor_id,
            db_tx_max_retries,
        })
    }
}
