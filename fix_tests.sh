cat << 'INNER_EOF' >> tests/customer_auth.rs

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

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
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
    
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["customer_id"].as_str().unwrap(), customer_a.to_string());
    
    let db_address = sqlx::query!("SELECT customer_id FROM customer_addresses WHERE id = $1", Uuid::parse_str(json["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool).await.unwrap();
    assert_eq!(db_address.customer_id, customer_a);

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
    
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    
    // Server must ignore the injected customer_id and use auth context
    assert_eq!(json["customer_id"].as_str().unwrap(), customer_a.to_string());
    let db_address = sqlx::query!("SELECT customer_id FROM customer_addresses WHERE id = $1", Uuid::parse_str(json["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool).await.unwrap();
    assert_eq!(db_address.customer_id, customer_a);
}
INNER_EOF
