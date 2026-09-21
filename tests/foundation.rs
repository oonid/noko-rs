use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use noko_rs::{AppState, app::build_router, config::Config};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn config_requires_database_url() {
    let _guard = ENV_LOCK.lock().await;
    unsafe { std::env::remove_var("DATABASE_URL") };
    assert!(Config::from_env().is_err());
}

#[tokio::test]
async fn config_loads_defaults_and_env_vars() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test",
        );
        std::env::remove_var("BIND_ADDR");
        std::env::remove_var("AUTH_MODE");
        std::env::remove_var("NOCODB_SERVICE_TOKEN");
        std::env::remove_var("NOCODB_SERVICE_ACTOR_ID");
        std::env::remove_var("DB_TX_MAX_RETRIES");
    }

    let config = Config::from_env().expect("config should load with DATABASE_URL set");
    assert_eq!(
        config.database_url,
        "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test"
    );
    assert_eq!(config.bind_addr, "0.0.0.0:3000");
    assert_eq!(config.auth_mode, "dev");
    assert_eq!(config.nocodb_service_token, None);
    assert_eq!(config.nocodb_service_actor_id, None);
    assert_eq!(config.db_tx_max_retries, 3);
}

#[tokio::test]
async fn config_loads_custom_env_vars() {
    let _guard = ENV_LOCK.lock().await;
    let actor_id = uuid::Uuid::new_v4();
    unsafe {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test",
        );
        std::env::set_var("BIND_ADDR", "127.0.0.1:8080");
        std::env::set_var("AUTH_MODE", "dev_header");
        std::env::set_var("NOCODB_SERVICE_TOKEN", "secret-token");
        std::env::set_var("NOCODB_SERVICE_ACTOR_ID", actor_id.to_string());
        std::env::set_var("DB_TX_MAX_RETRIES", "5");
    }

    let config = Config::from_env().expect("config should load with custom env vars");
    assert_eq!(config.bind_addr, "127.0.0.1:8080");
    assert_eq!(config.auth_mode, "dev_header");
    assert_eq!(config.nocodb_service_token.as_deref(), Some("secret-token"));
    assert_eq!(config.nocodb_service_actor_id, Some(actor_id));
    assert_eq!(config.db_tx_max_retries, 5);
}

#[tokio::test]
async fn config_rejects_invalid_values() {
    let _guard = ENV_LOCK.lock().await;
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://localhost/test");
        std::env::set_var("NOCODB_SERVICE_ACTOR_ID", "not-a-uuid");
    }
    assert!(Config::from_env().is_err());

    unsafe {
        std::env::remove_var("NOCODB_SERVICE_ACTOR_ID");
        std::env::set_var("DB_TX_MAX_RETRIES", "not-a-number");
    }
    assert!(Config::from_env().is_err());
}

#[tokio::test]
async fn health_live_returns_200_and_propagates_request_id() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://noko_test:noko_test@127.0.0.1:5432/noko_test")
        .expect("lazy pool creation");
    let config = Arc::new(Config {
        database_url: "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string(),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 3,
    });
    let state = AppState { pool, config };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/health/live")
        .header("x-request-id", "test-request-id-123")
        .body(Body::empty())
        .expect("valid request");

    let response = app.oneshot(req).await.expect("route execution");
    assert_eq!(response.status(), StatusCode::OK);

    let request_id = response.headers().get("x-request-id");
    assert_eq!(
        request_id.and_then(|v| v.to_str().ok()),
        Some("test-request-id-123")
    );
}

#[tokio::test]
async fn health_live_generates_request_id_if_absent() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://noko_test:noko_test@127.0.0.1:5432/noko_test")
        .expect("lazy pool creation");
    let config = Arc::new(Config {
        database_url: "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string(),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 3,
    });
    let state = AppState { pool, config };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/health/live")
        .body(Body::empty())
        .expect("valid request");

    let response = app.oneshot(req).await.expect("route execution");
    assert_eq!(response.status(), StatusCode::OK);

    let request_id = response.headers().get("x-request-id");
    assert!(request_id.is_some());
}

#[tokio::test]
async fn health_ready_returns_503_when_database_unreachable() {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(150))
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:54321/invalid")
        .expect("lazy pool creation");
    let config = Arc::new(Config {
        database_url: "postgres://invalid:invalid@127.0.0.1:54321/invalid".to_string(),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 3,
    });
    let state = AppState { pool, config };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/health/ready")
        .body(Body::empty())
        .expect("valid request");

    let response = app.oneshot(req).await.expect("route execution");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn health_ready_returns_200_when_database_connected() {
    let _guard = ENV_LOCK.lock().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());

    let pool = noko_rs::db::create_pool(&db_url)
        .await
        .expect("failed to connect to test database");

    let config = Arc::new(Config {
        database_url: db_url,
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 3,
    });
    let state = AppState { pool, config };
    let app = build_router(state);

    let req = Request::builder()
        .uri("/health/ready")
        .body(Body::empty())
        .expect("valid request");

    let response = app.oneshot(req).await.expect("route execution");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn binary_starts_and_gracefully_shuts_down() {
    let _guard = ENV_LOCK.lock().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noko-rs"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("DATABASE_URL", &db_url)
        .env("BIND_ADDR", "127.0.0.1:39123")
        .spawn()
        .expect("start binary");

    tokio::time::sleep(Duration::from_millis(600)).await;

    // Trigger graceful shutdown via SIGTERM
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();

    let exit_status = child.wait().expect("child exit");
    assert!(exit_status.success());
}
