cat << 'INNER_EOF' >> src/config.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Helper macro to isolate environment variables in tests
    // A bit tricky in concurrent tests, but we can set and unset and use serial tests,
    // or just use `temp_env` crate if available.
    // Instead of real environment, since we can't easily isolate, we just temporarily set it.
    // But since cargo test runs in parallel, mutating env can cause flakes.
    // However, the instructions say "Add unit tests for Config::from_env" so let's do it and hope there aren't parallel conflicts,
    // or we use a mutex if necessary. Wait, `serial_test` is standard for this, but maybe not in dependencies.
    // Let's just set it carefully.
    
    // Better yet, just use a mutex.
    lazy_static::lazy_static! {
        static ref ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    }

    fn setup_env() -> std::sync::MutexGuard<'static, ()> {
        let guard = ENV_MUTEX.lock().unwrap();
        env::set_var("DATABASE_URL", "postgres://test");
        guard
    }

    #[test]
    fn test_auth_mode_absent() {
        let _guard = setup_env();
        env::remove_var("AUTH_MODE");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Missing(key) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_dev() {
        let _guard = setup_env();
        env::set_var("AUTH_MODE", "dev");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(key, _) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_unsupported() {
        let _guard = setup_env();
        env::set_var("AUTH_MODE", "unsupported");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(key, _) if key == "AUTH_MODE"));
    }

    #[test]
    fn test_auth_mode_dev_header() {
        let _guard = setup_env();
        env::set_var("AUTH_MODE", "dev_header");
        let config = Config::from_env().unwrap();
        assert_eq!(config.auth_mode, "dev_header");
    }
}
INNER_EOF
