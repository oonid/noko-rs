use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use noko_rs::{AppState, app::build_router, config::Config};

use serde_json::json;

use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    async fn acquire() -> Self {
        Self {
            _lock: ENV_LOCK.lock().await,
        }
    }
}

async fn setup_app(
    pool: sqlx::PgPool,
    service_actor_id: Uuid,
    service_token: &str,
) -> axum::Router {
    let config = Arc::new(Config {
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string()
        }),
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
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'service_actor')").bind(actor_id).bind(format!("sub_{}", actor_id))
    .execute(&pool)
    .await
    .unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', $2, true) ON CONFLICT (code) DO UPDATE SET active = true").bind(location_id).bind("Main").execute(&pool).await.unwrap();
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": format!("SKU_{}", Uuid::new_v4()),
        "title": "Test Variant",
        "amount": 10050
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_ops_authentication_failures() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();
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
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "AUTH_REQUIRED");
    assert_eq!(body["retryable"], false);

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
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "INVALID_TOKEN");
    assert_eq!(body["retryable"], false);
}

#[tokio::test]
async fn atomicity_test_rollback_on_inventory_failure() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'service_actor')").bind(actor_id).bind(format!("sub_{}", actor_id))
    .execute(&pool)
    .await
    .unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', $2, true) ON CONFLICT (code) DO UPDATE SET active = true").bind(location_id).bind("Main").execute(&pool).await.unwrap();
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();

    let sku = format!("SKU_ATOMIC_{}", Uuid::new_v4());

    // Create trigger to fail inventory insert
    sqlx::query(
        "
        CREATE OR REPLACE FUNCTION fail_inventory_item() RETURNS trigger AS $$
        BEGIN
            RAISE EXCEPTION 'Intentional failure for atomicity test';
        END;
        $$ LANGUAGE plpgsql;
    ",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "
        CREATE TRIGGER trigger_fail_inventory_item
        BEFORE INSERT ON inventory_items
        FOR EACH ROW EXECUTE FUNCTION fail_inventory_item();
    ",
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": sku,
        "title": "Test Variant",
        "amount": 10050
    });

    let c_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels")
        .fetch_one(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let a_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(c_variants, a_variants);
    assert_eq!(c_prices, a_prices);
    assert_eq!(c_items, a_items);
    assert_eq!(c_levels, a_levels);

    // Drop trigger
    sqlx::query("DROP TRIGGER IF EXISTS trigger_fail_inventory_item ON inventory_items")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS fail_inventory_item()")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_adjust_inventory_success() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv')").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN2', 'Main2', true) ON CONFLICT (code) DO NOTHING").bind(location_id).execute(&pool).await.unwrap();
    let location_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM inventory_locations LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();

    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, $3, 'var', true)").bind(variant_id).bind(product_id).bind(format!("SKU_{}", Uuid::new_v4())).execute(&pool).await.unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 0, 0)").bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "inventory_item_id": item_id,
        "location_id": location_id,
        "delta": 5,
        "reason": "restock",
        "initiator_external_ref": "ext_123"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/inventory/adjustments")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify audit actor
    let audit_actor = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT actor_id FROM inventory_adjustments WHERE inventory_item_id = $1 LIMIT 1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_actor, Some(actor_id));
}

#[tokio::test]
async fn test_adjust_inventory_zero_delta() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "inventory_item_id": Uuid::new_v4(),
        "location_id": Uuid::new_v4(),
        "delta": 0,
        "reason": "restock"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/inventory/adjustments")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY); // 422

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "INVALID_INVENTORY_DELTA");
    assert_eq!(body["retryable"], false);
}

#[tokio::test]
async fn test_ops_authentication_missing_or_invalid_actor() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let missing_actor_id = Uuid::new_v4();
    let app_missing = setup_app(pool.clone(), missing_actor_id, "test_token").await;

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app_missing.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "SERVICE_ACTOR_NOT_FOUND");
    assert_eq!(body["retryable"], false);

    let inactive_actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, false, 'inactive')").bind(inactive_actor_id).bind(format!("sub_{}", inactive_actor_id)).execute(&pool).await.unwrap();
    let app_inactive = setup_app(pool.clone(), inactive_actor_id, "test_token").await;
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app_inactive.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "ACTOR_INACTIVE");
    assert_eq!(body["retryable"], false);

    let human_actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'human', $2, true, 'human')").bind(human_actor_id).bind(format!("sub_{}", human_actor_id)).execute(&pool).await.unwrap();
    let app_human = setup_app(pool.clone(), human_actor_id, "test_token").await;
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app_human.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "ACTOR_NOT_SERVICE");
    assert_eq!(body["retryable"], false);

    let agent_actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'agent', $2, true, 'agent')").bind(agent_actor_id).bind(format!("sub_{}", agent_actor_id)).execute(&pool).await.unwrap();
    let app_agent = setup_app(pool.clone(), agent_actor_id, "test_token").await;
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app_agent.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "ACTOR_NOT_SERVICE");
    assert_eq!(body["retryable"], false);
}

#[tokio::test]
async fn test_create_variant_negative_price() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": Uuid::new_v4(),
        "sku": "SKU_NEG_PRICE",
        "title": "Test Variant",
        "amount": -100
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY); // 422

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "INVALID_PRICE_AMOUNT");
    assert_eq!(body["retryable"], false);
}

#[tokio::test]
async fn test_create_variant_duplicate_sku() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', 'Main', true) ON CONFLICT (code) DO UPDATE SET active = true").bind(location_id).execute(&pool).await.unwrap();

    let sku = format!("SKU_DUP_{}", Uuid::new_v4());

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": sku,
        "title": "Test Variant",
        "amount": 100
    });

    // First request should succeed
    let req1 = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    // Second request with same SKU should fail with 409
    let req2 = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT); // 409

    let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "SKU_ALREADY_EXISTS");
}

#[tokio::test]
async fn test_create_variant_main_inactive() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', 'Main', false) ON CONFLICT (code) DO UPDATE SET active = false").bind(location_id).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": format!("SKU_INACTIVE_MAIN_{}", Uuid::new_v4()),
        "title": "Test Variant",
        "amount": 100
    });

    let c_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    let c_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels")
        .fetch_one(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "LOCATION_NOT_FOUND");
    assert_eq!(body["retryable"], false);

    let a_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(c_variants, a_variants);
    assert_eq!(c_prices, a_prices);
    assert_eq!(c_items, a_items);
    assert_eq!(c_levels, a_levels);

    sqlx::query("UPDATE inventory_locations SET active = true WHERE code = 'MAIN'")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_create_variant_product_not_found() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": Uuid::new_v4(), // not inserted
        "sku": format!("SKU_NOT_FOUND_{}", Uuid::new_v4()),
        "title": "Test Variant",
        "amount": 100
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "PRODUCT_NOT_FOUND");
}
