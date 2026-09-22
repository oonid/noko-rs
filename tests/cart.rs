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
            "postgres://noko_test:noko_test@127.0.0.1:5433/noko_test".to_string()
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
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth1', 'test')").bind(actor_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@b.com', 'A', 'B')").bind(customer_id).bind(actor_id).execute(&pool).await.unwrap();

    // Create a product, variant and price
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'SKU1', 'Var', true)").bind(variant_id).bind(product_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO variant_prices (variant_id, currency_code, amount) VALUES ($1, 'IDR', 1000)",
    )
    .bind(variant_id)
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
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(inv_item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO inventory_levels (inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, 10, 0)").bind(inv_item_id).bind(loc_id).execute(&pool).await.unwrap();

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

// --- NEW TESTS ---

#[sqlx::test(migrations = "./migrations")]
async fn test_rejection_normalization(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Create an actor and customer
    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth2', 'test2')").bind(actor_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a2@b.com', 'A', 'B')").bind(customer_id).bind(actor_id).execute(&pool).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth2")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
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

    // 1. Malformed JSON
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth2")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ invalid json "))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_body["retryable"], false);

    // 2. Malformed UUID in path
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts/not-a-uuid/items")
        .header("x-dev-auth-subject", "auth2")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": Uuid::new_v4(), "quantity": 1 }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_body["retryable"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_migration_constraints(pool: PgPool) {
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth3', 'test3')").bind(customer_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a3@b.com', 'A', 'B')").bind(customer_id).bind(customer_id).execute(&pool).await.unwrap();

    // Test invalid currency code
    let res = sqlx::query(
        "INSERT INTO carts (customer_id, currency_code, status) VALUES ($1, 'USD', 'active')",
    )
    .bind(customer_id)
    .execute(&pool)
    .await;
    assert!(res.is_err());

    // Test invalid status
    let res = sqlx::query(
        "INSERT INTO carts (customer_id, currency_code, status) VALUES ($1, 'IDR', 'pending')",
    )
    .bind(customer_id)
    .execute(&pool)
    .await;
    assert!(res.is_err());

    let cart_id = Uuid::new_v4();
    sqlx::query("INSERT INTO carts (id, customer_id, currency_code, status) VALUES ($1, $2, 'IDR', 'active')").bind(cart_id).bind(customer_id).execute(&pool).await.unwrap();

    // Test quantity <= 0
    let res = sqlx::query(
        "INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price) VALUES ($1, $2, 'T', 'S', 0, 1000)"
    ).bind(cart_id).bind(Uuid::new_v4()).execute(&pool).await;
    assert!(res.is_err());

    // Test unit_price < 0
    let res = sqlx::query(
        "INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price) VALUES ($1, $2, 'T', 'S', 1, -100)"
    ).bind(cart_id).bind(Uuid::new_v4()).execute(&pool).await;
    assert!(res.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_concurrency_create_active_cart(pool: PgPool) {
    use noko_rs::cart::repository::create_or_get_active_cart;
    use std::sync::Arc;

    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth4', 'test4')").bind(customer_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a4@b.com', 'A', 'B')").bind(customer_id).bind(customer_id).execute(&pool).await.unwrap();

    let pool_arc = Arc::new(pool.clone());
    let mut handles = vec![];

    for _ in 0..5 {
        let pool_c = pool_arc.clone();
        let cid = customer_id;
        handles.push(tokio::spawn(async move {
            let mut conn = pool_c.acquire().await.unwrap();
            create_or_get_active_cart(&mut conn, cid).await.unwrap()
        }));
    }

    let mut cart_ids = vec![];
    for h in handles {
        let cart = h.await.unwrap();
        cart_ids.push(cart.id);
    }

    let first_id = cart_ids[0];
    for id in cart_ids {
        assert_eq!(id, first_id);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cart_ownership_403(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Customer A
    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_a', 'test_a')").bind(a_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a@b.com', 'A', 'B')").bind(a_id).bind(a_id).execute(&pool).await.unwrap();

    // Customer B
    let b_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_b', 'test_b')").bind(b_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'b@b.com', 'A', 'B')").bind(b_id).bind(b_id).execute(&pool).await.unwrap();

    // Create cart for B
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_b")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let cart_b_id = body
        .get("cart")
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap();

    // A tries to add to B's cart
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_b_id))
        .header("x-dev-auth-subject", "auth_a")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": Uuid::new_v4(), "quantity": 1 }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_completed_cart_mutation_409(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let c_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_c', 'test_c')").bind(c_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'c@b.com', 'C', 'D')").bind(c_id).bind(c_id).execute(&pool).await.unwrap();

    // Create cart
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_c")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
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

    // Complete cart manually
    let parsed_id = Uuid::parse_str(cart_id).unwrap();
    sqlx::query("UPDATE carts SET status = 'completed' WHERE id = $1")
        .bind(parsed_id)
        .execute(&pool)
        .await
        .unwrap();

    // Try to mutate
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_c")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": Uuid::new_v4(), "quantity": 1 }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_body["code"], "CART_COMPLETED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_shipping_address_copy_proof(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let c_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_d', 'test_d')").bind(c_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'd@b.com', 'D', 'E')").bind(c_id).bind(c_id).execute(&pool).await.unwrap();

    // Create customer address
    let ca_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO customer_addresses (id, customer_id, label, recipient_name, phone, address_line_1, city, province, postal_code, country_code, is_default) VALUES ($1, $2, 'Home', 'John', '12345', 'Street 1', 'City', 'Prov', '12345', 'ID', true)"
    ).bind(ca_id).bind(c_id).execute(&pool).await.unwrap();

    // Create cart
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_d")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
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

    // Set cart address
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/shipping-address", cart_id))
        .header("x-dev-auth-subject", "auth_d")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "customer_address_id": ca_id }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Modify customer address
    sqlx::query("UPDATE customer_addresses SET recipient_name = 'Jane' WHERE id = $1")
        .bind(ca_id)
        .execute(&pool)
        .await
        .unwrap();

    // Fetch cart address
    let parsed_id = Uuid::parse_str(cart_id).unwrap();
    let addr_name: String =
        sqlx::query_scalar("SELECT recipient_name FROM cart_addresses WHERE cart_id = $1")
            .bind(parsed_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(addr_name, "John");
}
