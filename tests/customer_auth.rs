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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], "AUTH_REQUIRED");
    assert_eq!(json["retryable"], false);
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], "AUTH_SUBJECT_UNKNOWN");
    assert_eq!(json["retryable"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_inactive_actor(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO actors (id, kind, auth_subject, display_name, active) VALUES ($1, 'human', 'inactive_user', 'Inactive', false)"
    ).bind(actor_id)
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], "ACTOR_INACTIVE");
    assert_eq!(json["retryable"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_auth_header_not_a_customer(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'service', 'service_user', 'Service')"
    ).bind(actor_id)
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], "CUSTOMER_REQUIRED");
    assert_eq!(json["retryable"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cross_customer_isolation(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Customer A
    let actor_a = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_a', 'Customer A')").bind(actor_a).execute(&pool).await.unwrap();
    let customer_a = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@example.com', 'A', 'Customer')").bind(customer_a).bind(actor_a).execute(&pool).await.unwrap();

    // Customer B
    let actor_b = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_b', 'Customer B')").bind(actor_b).execute(&pool).await.unwrap();
    let customer_b = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'b@example.com', 'B', 'Customer')").bind(customer_b).bind(actor_b).execute(&pool).await.unwrap();

    // Address for A
    let address_a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customer_addresses (id, customer_id, label, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, $2, 'Home A', 'A', '123 A St', 'City', 'Prov', '12345', 'US')").bind(address_a_id).bind(customer_a).execute(&pool).await.unwrap();

    // Address for B
    let address_b_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customer_addresses (id, customer_id, label, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, $2, 'Home B', 'B', '456 B St', 'City', 'Prov', '67890', 'US')").bind(address_b_id).bind(customer_b).execute(&pool).await.unwrap();

    // Check A sees Address A, not B
    let req_a = Request::builder()
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_a")
        .body(Body::empty())
        .unwrap();
    let res_a = app.clone().oneshot(req_a).await.unwrap();
    assert_eq!(res_a.status(), StatusCode::OK);
    let bytes_a = axum::body::to_bytes(res_a.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_a: serde_json::Value = serde_json::from_slice(&bytes_a).unwrap();
    let addrs_a = json_a.as_array().unwrap();
    assert_eq!(addrs_a.len(), 1);
    assert_eq!(addrs_a[0]["id"].as_str().unwrap(), address_a_id.to_string());

    // Check B sees Address B, not A
    let req_b = Request::builder()
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_b")
        .body(Body::empty())
        .unwrap();
    let res_b = app.oneshot(req_b).await.unwrap();
    assert_eq!(res_b.status(), StatusCode::OK);
    let bytes_b = axum::body::to_bytes(res_b.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_b: serde_json::Value = serde_json::from_slice(&bytes_b).unwrap();
    let addrs_b = json_b.as_array().unwrap();
    assert_eq!(addrs_b.len(), 1);
    assert_eq!(addrs_b[0]["id"].as_str().unwrap(), address_b_id.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inventory_adjustments_actor_id(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO products (id, title, description, status) VALUES ($1, 'P1', 'p1', 'active')",
    )
    .bind(product_id)
    .execute(&pool)
    .await
    .unwrap();

    let variant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'SKU1', 'V1')",
    )
    .bind(variant_id)
    .bind(product_id)
    .execute(&pool)
    .await
    .unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    let loc_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name) VALUES ($1, 'TEST', 'Test')")
        .bind(loc_id)
        .execute(&pool)
        .await
        .unwrap();

    // Valid NULL
    sqlx::query("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason) VALUES ($1, $2, 10, 'manual')").bind(item_id).bind(loc_id).execute(&pool).await.unwrap();

    // Valid Actor
    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_actor', 'Actor')").bind(actor_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason, actor_id) VALUES ($1, $2, 10, 'manual', $3)").bind(item_id).bind(loc_id).bind(actor_id).execute(&pool).await.unwrap();

    // Invalid FK
    let invalid_actor = Uuid::new_v4();
    let err = sqlx::query("INSERT INTO inventory_adjustments (inventory_item_id, location_id, delta, reason, actor_id) VALUES ($1, $2, 10, 'manual', $3)").bind(item_id).bind(loc_id).bind(invalid_actor).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("inventory_adjustments_actor_fk"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_store_me_positive(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_a = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_a', 'Customer A')").bind(actor_a).execute(&pool).await.unwrap();
    let customer_a = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@example.com', 'A', 'Customer')").bind(customer_a).bind(actor_a).execute(&pool).await.unwrap();

    let req = Request::builder()
        .uri("/store/me")
        .header("X-Dev-Auth-Subject", "subject_a")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["id"].as_str().unwrap(), customer_a.to_string());
    assert_eq!(json["actor_id"].as_str().unwrap(), actor_a.to_string());
    assert_eq!(json["email"].as_str().unwrap(), "a@example.com");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_post_store_me_addresses_ownership(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let actor_a = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_a', 'Customer A')").bind(actor_a).execute(&pool).await.unwrap();
    let customer_a = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@example.com', 'A', 'Customer')").bind(customer_a).bind(actor_a).execute(&pool).await.unwrap();

    let actor_b = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_b', 'Customer B')").bind(actor_b).execute(&pool).await.unwrap();
    let customer_b = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'b@example.com', 'B', 'Customer')").bind(customer_b).bind(actor_b).execute(&pool).await.unwrap();

    // 1. Valid creation
    let body = serde_json::json!({
        "label": "Work",
        "recipient_name": "A",
        "address_line_1": "123 Work St",
        "city": "City",
        "province": "Prov",
        "postal_code": "12345",
        "country_code": "US"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_a")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json["customer_id"].as_str().unwrap(),
        customer_a.to_string()
    );

    let db_address =
        sqlx::query_scalar::<_, Uuid>("SELECT customer_id FROM customer_addresses WHERE id = $1")
            .bind(Uuid::parse_str(json["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(db_address, customer_a);

    // 2. Malicious client
    let malicious_body = serde_json::json!({
        "customer_id": customer_b.to_string(),
        "label": "Work2",
        "recipient_name": "A",
        "address_line_1": "123 Work St",
        "city": "City",
        "province": "Prov",
        "postal_code": "12345",
        "country_code": "US"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/store/me/addresses")
        .header("X-Dev-Auth-Subject", "subject_a")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&malicious_body).unwrap()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // Server must ignore the injected customer_id and use auth context
    assert_eq!(
        json["customer_id"].as_str().unwrap(),
        customer_a.to_string()
    );
    let db_address =
        sqlx::query_scalar::<_, Uuid>("SELECT customer_id FROM customer_addresses WHERE id = $1")
            .bind(Uuid::parse_str(json["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(db_address, customer_a);
}
