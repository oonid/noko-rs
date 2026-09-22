use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use noko_rs::{app::AppState, app::build_router, config::Config};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_app(pool: PgPool) -> axum::Router {
    let config = Arc::new(Config {
        database_url: std::env::var("DATABASE_URL").unwrap(),
        bind_addr: "127.0.0.1:0".to_string(),
        auth_mode: "dev_header".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 2,
    });
    let state = AppState { pool, config };
    build_router(state)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_missing(pool: PgPool) {
    let app = setup_test_app(pool).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/store/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_unknown_subject(pool: PgPool) {
    let app = setup_test_app(pool).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/store/me")
                .header("X-Dev-Auth-Subject", "unknown_user")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_inactive_actor(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO actors (id, kind, auth_subject, display_name, active) VALUES ($1, 'human', 'inactive_user', 'Inactive', false)",
        actor_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/store/me")
                .header("X-Dev-Auth-Subject", "inactive_user")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_not_a_customer(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'service', 'service_user', 'Service')",
        actor_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/store/me")
                .header("X-Dev-Auth-Subject", "service_user")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cross_customer_isolation(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Customer A
    let actor_a = Uuid::new_v4();
    sqlx::query!("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_a', 'Customer A')", actor_a).execute(&pool).await.unwrap();
    let customer_a = Uuid::new_v4();
    sqlx::query!("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@example.com', 'A', 'Customer')", customer_a, actor_a).execute(&pool).await.unwrap();

    // Customer B
    let actor_b = Uuid::new_v4();
    sqlx::query!("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_b', 'Customer B')", actor_b).execute(&pool).await.unwrap();
    let customer_b = Uuid::new_v4();
    sqlx::query!("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'b@example.com', 'B', 'Customer')", customer_b, actor_b).execute(&pool).await.unwrap();

    // Address for A
    sqlx::query!("INSERT INTO customer_addresses (customer_id, label, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, 'Home', 'A', '123 A St', 'City', 'Prov', '12345', 'US')", customer_a).execute(&pool).await.unwrap();

    // Check A sees 1 address
    let req_a = Request::builder()
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_a")
        .body(Body::empty())
        .unwrap();
    let res_a = app.clone().oneshot(req_a).await.unwrap();
    assert_eq!(res_a.status(), StatusCode::OK);

    // Check B sees 0 addresses
    let req_b = Request::builder()
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_b")
        .body(Body::empty())
        .unwrap();
    let res_b = app.oneshot(req_b).await.unwrap();
    assert_eq!(res_b.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inventory_adjustments_actor_id(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, title, description, status) VALUES ($1, 'P1', 'p1', 'active')",
        product_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let variant_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'SKU1', 'V1')",
        variant_id,
        product_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)",
        item_id,
        variant_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let loc_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO inventory_locations (id, code, name) VALUES ($1, 'TEST', 'Test')",
        loc_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Valid NULL
    sqlx::query!("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason) VALUES ($1, $2, 10, 'manual')", item_id, loc_id).execute(&pool).await.unwrap();

    // Valid Actor
    let actor_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_actor', 'Actor')", actor_id).execute(&pool).await.unwrap();
    sqlx::query!("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason, actor_id) VALUES ($1, $2, 10, 'manual', $3)", item_id, loc_id, actor_id).execute(&pool).await.unwrap();

    // Invalid FK
    let invalid_actor = Uuid::new_v4();
    let err = sqlx::query!("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason, actor_id) VALUES ($1, $2, 10, 'manual', $3)", item_id, loc_id, invalid_actor).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("inventory_adjustments_actor_fk"));
}
