import os

with open("tests/operations.rs", "w") as f:
    f.write(r"""
use axum::{body::Body, http::{Request, StatusCode}};
use noko_rs::{AppState, app::build_router, config::Config};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    async fn acquire() -> Self {
        Self { _lock: ENV_LOCK.lock().await }
    }
}

async fn setup_app(
    pool: sqlx::PgPool,
    service_actor_id: Uuid,
    service_token: &str,
) -> axum::Router {
    let config = Arc::new(Config {
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string()),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev_header".to_string(),
        nocodb_service_token: Some(service_token.to_string()),
        nocodb_service_actor_id: Some(service_actor_id),
        db_tx_max_retries: 2,
    });
    let state = AppState { pool, config };
    build_router(state)
}

#[tokio::test]
async fn test_create_variant_success() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO actors (id, kind, auth_subject, active) VALUES ($1, 'service', $2, true)",
        actor_id,
        format!("sub_{}", actor_id)
    ).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, sku, title, is_active) VALUES ($1, $2, 'test product', true)",
        product_id,
        format!("PROD_{}", product_id)
    ).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": format!("SKU_{}", Uuid::new_v4()),
        "title": "Test Variant",
        "amount": "100.50"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_ops_authentication_failures() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    let actor_id = Uuid::new_v4();
    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    // Missing token
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Wrong token
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer wrong_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn atomicity_test_rollback_on_inventory_failure() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO actors (id, kind, auth_subject, active) VALUES ($1, 'service', $2, true)",
        actor_id,
        format!("sub_{}", actor_id)
    ).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, sku, title, is_active) VALUES ($1, $2, 'test product', true)",
        product_id,
        format!("PROD_{}", product_id)
    ).execute(&pool).await.unwrap();

    let sku = format!("SKU_ATOMIC_{}", Uuid::new_v4());

    // Create trigger to fail inventory insert
    sqlx::query("
        CREATE OR REPLACE FUNCTION fail_inventory_item() RETURNS trigger AS $$
        BEGIN
            RAISE EXCEPTION 'Intentional failure for atomicity test';
        END;
        $$ LANGUAGE plpgsql;
    ").execute(&pool).await.unwrap();
    sqlx::query("
        CREATE TRIGGER trigger_fail_inventory_item
        BEFORE INSERT ON inventory_items
        FOR EACH ROW EXECUTE FUNCTION fail_inventory_item();
    ").execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": sku,
        "title": "Test Variant",
        "amount": "100.50"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Drop trigger
    sqlx::query("DROP TRIGGER IF EXISTS trigger_fail_inventory_item ON inventory_items").execute(&pool).await.unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS fail_inventory_item()").execute(&pool).await.unwrap();

    // Verify Variant and Price are ABSENT
    let variant_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM product_variants WHERE sku = $1)")
        .bind(&sku)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!variant_exists, "Variant should have been rolled back");
}
""")
