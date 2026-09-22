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

// 9A Additions
#[sqlx::test(migrations = "./migrations")]
async fn test_migration_constraints_extended(pool: PgPool) {
    let c_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'a_xt', 't')").bind(c_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'x@b.com', 'A', 'B')").bind(c_id).bind(c_id).execute(&pool).await.unwrap();

    let cart_id = Uuid::new_v4();
    sqlx::query("INSERT INTO carts (id, customer_id, currency_code, status) VALUES ($1, $2, 'IDR', 'active')").bind(cart_id).bind(c_id).execute(&pool).await.unwrap();

    // Two active carts rejected
    let res = sqlx::query("INSERT INTO carts (id, customer_id, currency_code, status) VALUES ($1, $2, 'IDR', 'active')").bind(Uuid::new_v4()).bind(c_id).execute(&pool).await;
    assert!(res.is_err());

    let p_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')")
        .bind(p_id)
        .execute(&pool)
        .await
        .unwrap();
    let v_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'S', 'T', true)").bind(v_id).bind(p_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price) VALUES ($1, $2, 'T', 'S', 1, 1000)").bind(cart_id).bind(v_id).execute(&pool).await.unwrap();

    // Duplicate variant rejected
    let res = sqlx::query("INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price) VALUES ($1, $2, 'T', 'S', 1, 1000)").bind(cart_id).bind(v_id).execute(&pool).await;
    assert!(res.is_err());

    // Duplicate address rejected (or upsert handled by ON CONFLICT, but the raw insert must conflict)
    sqlx::query("INSERT INTO cart_addresses (cart_id, kind, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, 'shipping', 'A', 'A', 'A', 'A', 'A', 'A')").bind(cart_id).execute(&pool).await.unwrap();
    let res = sqlx::query("INSERT INTO cart_addresses (cart_id, kind, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, 'shipping', 'B', 'B', 'B', 'B', 'B', 'B')").bind(cart_id).execute(&pool).await;
    assert!(res.is_err());

    // Invalid CartAddress kind rejected
    let res = sqlx::query("INSERT INTO cart_addresses (cart_id, kind, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, 'billing_and_shipping', 'A', 'A', 'A', 'A', 'A', 'A')").bind(cart_id).execute(&pool).await;
    assert!(res.is_err());
}

// 9B Update check for active carts count
#[sqlx::test(migrations = "./migrations")]
async fn test_active_cart_concurrency_count(pool: PgPool) {
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth5', 'test5')").bind(customer_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'a5@b.com', 'A', 'B')").bind(customer_id).bind(customer_id).execute(&pool).await.unwrap();

    let pool_arc = std::sync::Arc::new(pool.clone());
    let mut handles = vec![];
    for _ in 0..5 {
        let pool_c = pool_arc.clone();
        let cid = customer_id;
        handles.push(tokio::spawn(async move {
            let mut conn = pool_c.acquire().await.unwrap();
            noko_rs::cart::repository::create_or_get_active_cart(&mut conn, cid)
                .await
                .unwrap()
        }));
    }

    let mut cart_ids = vec![];
    for h in handles {
        cart_ids.push(h.await.unwrap().id);
    }

    for id in &cart_ids {
        assert_eq!(*id, cart_ids[0]);
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM carts WHERE customer_id = $1 AND status = 'active'",
    )
    .bind(customer_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

// 9C Cross-customer mutation
#[sqlx::test(migrations = "./migrations")]
async fn test_cross_customer_mutation_contract(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_aa', 'test')").bind(a_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'aa@b.com', 'A', 'B')").bind(a_id).bind(a_id).execute(&pool).await.unwrap();

    let b_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_bb', 'test')").bind(b_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'bb@b.com', 'A', 'B')").bind(b_id).bind(b_id).execute(&pool).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_bb")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_aa")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": Uuid::new_v4(), "quantity": 1 }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "CART_NOT_FOUND");
    assert_eq!(err["retryable"], false);
}

// 9D Completed cart mutation
#[sqlx::test(migrations = "./migrations")]
async fn test_completed_cart_mutation_contract(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;
    let c_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_cc', 'test')").bind(c_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'cc@b.com', 'A', 'B')").bind(c_id).bind(c_id).execute(&pool).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_cc")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    sqlx::query("UPDATE carts SET status = 'completed' WHERE id = $1")
        .bind(Uuid::parse_str(&cart_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_cc")
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
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "CART_COMPLETED");
    assert_eq!(err["retryable"], false);
}

// 9E & 9F Full CartItem snapshot proof and existing variant re-add
#[sqlx::test(migrations = "./migrations")]
async fn test_cart_item_snapshot_proof(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Setup data
    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_ss', 'test')").bind(a_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'ss@b.com', 'A', 'B')").bind(customer_id).bind(a_id).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'ORIGINAL-SKU', 'Original Title', true)").bind(variant_id).bind(product_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO variant_prices (variant_id, currency_code, amount) VALUES ($1, 'IDR', 1000)",
    )
    .bind(variant_id)
    .execute(&pool)
    .await
    .unwrap();
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

    // Create cart
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_ss")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 1. Add variant to cart
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_ss")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": variant_id, "quantity": 1 }).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // 2. Capture CartItem
    let item_snapshot = sqlx::query("SELECT variant_title, sku, unit_price, quantity FROM cart_items WHERE cart_id = $1 AND variant_id = $2").bind(Uuid::parse_str(&cart_id).unwrap()).bind(variant_id).fetch_one(&pool).await.unwrap();
    let variant_title: String = sqlx::Row::try_get(&item_snapshot, "variant_title").unwrap();
    let sku: String = sqlx::Row::try_get(&item_snapshot, "sku").unwrap();
    let unit_price: i64 = sqlx::Row::try_get(&item_snapshot, "unit_price").unwrap();
    let quantity: i64 = sqlx::Row::try_get(&item_snapshot, "quantity").unwrap();
    assert_eq!(variant_title, "Original Title");
    assert_eq!(sku, "ORIGINAL-SKU");
    assert_eq!(unit_price, 1000);
    assert_eq!(quantity, 1);

    // 3. Mutate source
    sqlx::query("UPDATE product_variants SET title = 'New Title', sku = 'NEW-SKU' WHERE id = $1")
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE variant_prices SET amount = 2000 WHERE variant_id = $1 AND currency_code = 'IDR'",
    )
    .bind(variant_id)
    .execute(&pool)
    .await
    .unwrap();

    // 4. Query again
    let item_snapshot2 = sqlx::query("SELECT variant_title, sku, unit_price FROM cart_items WHERE cart_id = $1 AND variant_id = $2").bind(Uuid::parse_str(&cart_id).unwrap()).bind(variant_id).fetch_one(&pool).await.unwrap();
    let variant_title: String = sqlx::Row::try_get(&item_snapshot2, "variant_title").unwrap();
    let sku: String = sqlx::Row::try_get(&item_snapshot2, "sku").unwrap();
    let unit_price: i64 = sqlx::Row::try_get(&item_snapshot2, "unit_price").unwrap();
    assert_eq!(variant_title, "Original Title");
    assert_eq!(sku, "ORIGINAL-SKU");
    assert_eq!(unit_price, 1000);

    // 5. POST same variant again
    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_ss")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": variant_id, "quantity": 1 }).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    // Assert quantity increments, snapshot unchanged
    let item_snapshot3 = sqlx::query("SELECT variant_title, sku, unit_price, quantity FROM cart_items WHERE cart_id = $1 AND variant_id = $2").bind(Uuid::parse_str(&cart_id).unwrap()).bind(variant_id).fetch_one(&pool).await.unwrap();
    let variant_title: String = sqlx::Row::try_get(&item_snapshot3, "variant_title").unwrap();
    let sku: String = sqlx::Row::try_get(&item_snapshot3, "sku").unwrap();
    let unit_price: i64 = sqlx::Row::try_get(&item_snapshot3, "unit_price").unwrap();
    let quantity: i64 = sqlx::Row::try_get(&item_snapshot3, "quantity").unwrap();
    assert_eq!(quantity, 2);
    assert_eq!(variant_title, "Original Title");
    assert_eq!(sku, "ORIGINAL-SKU");
    assert_eq!(unit_price, 1000);
}

// 9G & 9I Inventory reservation boundary & DELETE inventory neutrality
#[sqlx::test(migrations = "./migrations")]
async fn test_inventory_boundary(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    // Check no inventory_reservations table
    let regclass: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.inventory_reservations')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(regclass.is_none());

    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_ii', 'test')").bind(a_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'ii@b.com', 'A', 'B')").bind(customer_id).bind(a_id).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'SKU', 'Title', true)").bind(variant_id).bind(product_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO variant_prices (variant_id, currency_code, amount) VALUES ($1, 'IDR', 1000)",
    )
    .bind(variant_id)
    .execute(&pool)
    .await
    .unwrap();
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

    let before = sqlx::query("SELECT stocked_quantity, reserved_quantity FROM inventory_levels WHERE inventory_item_id = $1").bind(inv_item_id).fetch_one(&pool).await.unwrap();
    let b_sq: i64 = sqlx::Row::try_get(&before, "stocked_quantity").unwrap();
    let b_rq: i64 = sqlx::Row::try_get(&before, "reserved_quantity").unwrap();

    // Add cart
    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_ii")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_ii")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": variant_id, "quantity": 1 }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let item_id = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let after_add = sqlx::query("SELECT stocked_quantity, reserved_quantity FROM inventory_levels WHERE inventory_item_id = $1").bind(inv_item_id).fetch_one(&pool).await.unwrap();
    let aa_sq: i64 = sqlx::Row::try_get(&after_add, "stocked_quantity").unwrap();
    let aa_rq: i64 = sqlx::Row::try_get(&after_add, "reserved_quantity").unwrap();
    assert_eq!(b_sq, aa_sq);
    assert_eq!(b_rq, aa_rq);

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth_ii")
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap();

    let after_del = sqlx::query("SELECT stocked_quantity, reserved_quantity FROM inventory_levels WHERE inventory_item_id = $1").bind(inv_item_id).fetch_one(&pool).await.unwrap();
    let ad_sq: i64 = sqlx::Row::try_get(&after_del, "stocked_quantity").unwrap();
    let ad_rq: i64 = sqlx::Row::try_get(&after_del, "reserved_quantity").unwrap();
    assert_eq!(b_sq, ad_sq);
    assert_eq!(b_rq, ad_rq);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM cart_items WHERE id = $1")
        .bind(Uuid::parse_str(&item_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// 9H PATCH tests
#[sqlx::test(migrations = "./migrations")]
async fn test_patch_quantity(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;
    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_pp', 'test')").bind(a_id).execute(&pool).await.unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'pp@b.com', 'A', 'B')").bind(customer_id).bind(a_id).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'Prod', 'active')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title, active) VALUES ($1, $2, 'SKU', 'Title', true)").bind(variant_id).bind(product_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO variant_prices (variant_id, currency_code, amount) VALUES ($1, 'IDR', 1000)",
    )
    .bind(variant_id)
    .execute(&pool)
    .await
    .unwrap();
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

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_pp")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/items", cart_id))
        .header("x-dev-auth-subject", "auth_pp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "variant_id": variant_id, "quantity": 1 }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let item_id = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Patch positive
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth_pp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "quantity": 3 }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let patched: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(patched["quantity"].as_i64().unwrap(), 3);

    // Patch 0
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth_pp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "quantity": 0 }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "INVALID_QUANTITY");
    assert_eq!(err["retryable"], false);

    // Patch -1
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/store/carts/{}/items/{}", cart_id, item_id))
        .header("x-dev-auth-subject", "auth_pp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "quantity": -1 }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Invalid item_id from another cart
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/store/carts/{}/items/{}", cart_id, Uuid::new_v4()))
        .header("x-dev-auth-subject", "auth_pp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "quantity": 1 }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "CART_ITEM_NOT_FOUND");
}

// 9J CustomerAddress ownership
#[sqlx::test(migrations = "./migrations")]
async fn test_customer_address_ownership(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_aa1', 'test')").bind(a_id).execute(&pool).await.unwrap();
    let ca_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'aa1@b.com', 'A', 'B')").bind(ca_id).bind(a_id).execute(&pool).await.unwrap();

    let b_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_bb1', 'test')").bind(b_id).execute(&pool).await.unwrap();
    let cb_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'bb1@b.com', 'A', 'B')").bind(cb_id).bind(b_id).execute(&pool).await.unwrap();

    let addr_b_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customer_addresses (id, customer_id, label, recipient_name, phone, address_line_1, city, province, postal_code, country_code, is_default) VALUES ($1, $2, 'Home', 'B', '1', '1', '1', '1', '1', '1', true)").bind(addr_b_id).bind(cb_id).execute(&pool).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_aa1")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_a_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .method("POST")
        .uri(format!("/store/carts/{}/shipping-address", cart_a_id))
        .header("x-dev-auth-subject", "auth_aa1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "customer_address_id": addr_b_id }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "CUSTOMER_ADDRESS_NOT_FOUND");
    assert_eq!(err["retryable"], false);
}

// 9K GET current Cart
#[sqlx::test(migrations = "./migrations")]
async fn test_get_current_cart(pool: PgPool) {
    let app = setup_test_app(pool.clone()).await;
    let a_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'auth_get', 'test')").bind(a_id).execute(&pool).await.unwrap();
    let ca_id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'get@b.com', 'A', 'B')").bind(ca_id).bind(a_id).execute(&pool).await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/store/carts/current")
        .header("x-dev-auth-subject", "auth_get")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["code"], "CART_NOT_FOUND");
    assert_eq!(err["retryable"], false);

    let req = Request::builder()
        .method("POST")
        .uri("/store/carts")
        .header("x-dev-auth-subject", "auth_get")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .method("GET")
        .uri("/store/carts/current")
        .header("x-dev-auth-subject", "auth_get")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let cart_id2 = serde_json::from_slice::<Value>(&bytes).unwrap()["cart"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(cart_id, cart_id2);
}
