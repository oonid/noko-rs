cat << 'INNER_EOF' > src/config.rs
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
        let auth_mode =
            env::var("AUTH_MODE").map_err(|_| ConfigError::Missing("AUTH_MODE".to_string()))?;

        if auth_mode != "dev_header" {
            return Err(ConfigError::Invalid(
                "AUTH_MODE".to_string(),
                "only dev_header is supported".to_string(),
            ));
        }
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
            Ok(val) if !val.is_empty() => {
                let retries = val.parse::<u32>().map_err(|e| {
                    ConfigError::Invalid("DB_TX_MAX_RETRIES".to_string(), e.to_string())
                })?;
                if retries > 2 {
                    return Err(ConfigError::Invalid(
                        "DB_TX_MAX_RETRIES".to_string(),
                        "must be between 0 and 2".to_string(),
                    ));
                }
                retries
            }
            _ => 2,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
        old_db_url: Result<String, env::VarError>,
        old_auth_mode: Result<String, env::VarError>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Ok(v) = &self.old_db_url {
                    env::set_var("DATABASE_URL", v);
                } else {
                    env::remove_var("DATABASE_URL");
                }
                if let Ok(v) = &self.old_auth_mode {
                    env::set_var("AUTH_MODE", v);
                } else {
                    env::remove_var("AUTH_MODE");
                }
            }
        }
    }

    fn setup_env() -> EnvGuard {
        let guard = ENV_MUTEX.lock().unwrap();
        let old_db_url = env::var("DATABASE_URL");
        let old_auth_mode = env::var("AUTH_MODE");
        unsafe { env::set_var("DATABASE_URL", "postgres://test") };
        EnvGuard {
            _guard: guard,
            old_db_url,
            old_auth_mode,
        }
    }

    #[test]
    fn test_auth_mode_absent() {
        let _guard = setup_env();
        unsafe { env::remove_var("AUTH_MODE") };
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Missing(key) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_dev() {
        let _guard = setup_env();
        unsafe { env::set_var("AUTH_MODE", "dev") };
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(key, _) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_unsupported() {
        let _guard = setup_env();
        unsafe { env::set_var("AUTH_MODE", "unsupported") };
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(key, _) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_dev_header() {
        let _guard = setup_env();
        unsafe { env::set_var("AUTH_MODE", "dev_header") };
        let config = Config::from_env().unwrap();
        assert_eq!(config.auth_mode, "dev_header");
    }
}
INNER_EOF
