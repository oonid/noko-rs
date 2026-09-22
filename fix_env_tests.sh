sed -i 's/assert!(matches!(err, ConfigError::Missing(key) if key == "AUTH_MODE"));/assert_eq!(err.to_string(), "missing environment variable: AUTH_MODE");/g' src/config.rs
