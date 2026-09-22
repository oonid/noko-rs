use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use noko_rs::{app::AppState, app::build_router, config::Config};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_app(pool: PgPool) -> axum::Router {
    let config = Arc::new(Config {
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string()
        }),
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
async fn test_cart_full_workflow(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Create an actor and customer
    let actor_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth1', 'test')", actor_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@b.com', 'A', 'B')", customer_id, actor_id).execute(&pool).await.unwrap();

    // Create a product, variant and price
    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')",
        product_id
    )
    .execute(&pool)
    .await
    .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'SKU1', 'Var', true)", variant_id, product_id).execute(&pool).await.unwrap();
    sqlx::query!(
        "INSERT INTO variant_prices (variant_id, currency_code, amount) VALUES ($1, 'IDR', 1000)",
        variant_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Add inventory so we can add to cart
    let loc_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let inv_item_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)",
        inv_item_id,
        variant_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("INSERT INTO inventory_levels (inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, 10, 0)", inv_item_id, loc_id).execute(&pool).await.unwrap();

    // 1. Create cart
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth1")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let cart_id = body
        .get("cart")
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap();

    // 2. Add to cart
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": variant_id, "quantity": 1 }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let item_id = body.get("id").unwrap().as_str().unwrap();

    // 3. Update item
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "quantity": 2 }).to_string()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Remove item
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth1")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}
