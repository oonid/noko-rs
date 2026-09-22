import re

with open("tests/operations.rs", "r") as f:
    content = f.read()

# Issue 1: Add a test for active=false MAIN location
issue_1_test = """
#[tokio::test]
async fn test_create_variant_main_inactive() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
    let pool = noko_rs::db::create_pool(&db_url).await.unwrap();
    noko_rs::db::run_migrations(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'service', $2, true, 'srv') ON CONFLICT (id) DO NOTHING").bind(actor_id).bind(format!("sub_{}", actor_id)).execute(&pool).await.unwrap();

    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title, status) VALUES ($1, 'test product', 'active')").bind(product_id).execute(&pool).await.unwrap();

    let location_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', 'Main', false) ON CONFLICT (code) DO UPDATE SET active = false").bind(location_id).execute(&pool).await.unwrap();

    let app = setup_app(pool.clone(), actor_id, "test_token").await;

    let payload = json!({
        "product_id": product_id,
        "sku": format!("SKU_INACTIVE_MAIN_{}", Uuid::new_v4()),
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
    
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "LOCATION_NOT_FOUND");
    
    sqlx::query("UPDATE inventory_locations SET active = true WHERE code = 'MAIN'").execute(&pool).await.unwrap();
}
"""

content = content + issue_1_test

# Issue 4 test (add a test for PRODUCT_NOT_FOUND)
issue_4_test = """
#[tokio::test]
async fn test_create_variant_product_not_found() {
    let _guard = EnvGuard::acquire().await;
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string());
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
    
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "PRODUCT_NOT_FOUND");
}
"""
content = content + issue_4_test


# Issue 2: atomicity_test_rollback_on_inventory_failure
# Find the exact function `atomicity_test_rollback_on_inventory_failure` and replace inside it
def replace_in_func(func_name, old_str, new_str, text):
    start = text.find(f"async fn {func_name}()")
    if start == -1: return text
    end = text.find("async fn ", start + 20)
    if end == -1: end = len(text)
    sub = text[start:end]
    sub = sub.replace(old_str, new_str)
    return text[:start] + sub + text[end:]

old_atomic = """    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);"""

new_atomic = """    let c_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants").fetch_one(&pool).await.unwrap();
    let c_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices").fetch_one(&pool).await.unwrap();
    let c_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items").fetch_one(&pool).await.unwrap();
    let c_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels").fetch_one(&pool).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let a_variants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_variants").fetch_one(&pool).await.unwrap();
    let a_prices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variant_prices").fetch_one(&pool).await.unwrap();
    let a_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items").fetch_one(&pool).await.unwrap();
    let a_levels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_levels").fetch_one(&pool).await.unwrap();

    assert_eq!(c_variants, a_variants);
    assert_eq!(c_prices, a_prices);
    assert_eq!(c_items, a_items);
    assert_eq!(c_levels, a_levels);"""

content = replace_in_func("atomicity_test_rollback_on_inventory_failure", old_atomic, new_atomic, content)


# Issue 3: test_ops_authentication_missing_or_invalid_actor
old_auth_missing = """    let res = app_missing.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);"""
new_auth_missing = """    let res = app_missing.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "SERVICE_ACTOR_NOT_FOUND");"""
content = replace_in_func("test_ops_authentication_missing_or_invalid_actor", old_auth_missing, new_auth_missing, content)

old_auth_inactive = """    let res = app_inactive.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);"""
new_auth_inactive = """    let res = app_inactive.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "ACTOR_INACTIVE");"""
content = replace_in_func("test_ops_authentication_missing_or_invalid_actor", old_auth_inactive, new_auth_inactive, content)

old_auth_human = """    let res = app_human.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);"""
new_auth_human = """    let res = app_human.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "ACTOR_NOT_SERVICE");"""
content = replace_in_func("test_ops_authentication_missing_or_invalid_actor", old_auth_human, new_auth_human, content)


# Issue 3: test_ops_authentication_failures
old_fail_missing = """    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);"""
new_fail_missing = """    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "AUTH_REQUIRED");"""
# Need to replace the FIRST occurrence in test_ops_authentication_failures for missing, and SECOND for wrong
def replace_in_func_twice(func_name, old_str, new_str1, new_str2, text):
    start = text.find(f"async fn {func_name}()")
    if start == -1: return text
    end = text.find("async fn ", start + 20)
    if end == -1: end = len(text)
    sub = text[start:end]
    first_idx = sub.find(old_str)
    if first_idx == -1: return text
    sub = sub[:first_idx] + new_str1 + sub[first_idx+len(old_str):]
    
    second_idx = sub.find(old_str)
    if second_idx == -1: return text
    sub = sub[:second_idx] + new_str2 + sub[second_idx+len(old_str):]
    
    return text[:start] + sub + text[end:]

new_fail_wrong = """    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "INVALID_TOKEN");"""
content = replace_in_func_twice("test_ops_authentication_failures", old_fail_missing, new_fail_missing, new_fail_wrong, content)

# Issue 7: Duplicate SKU Test
old_dup = """    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT); // 409"""
new_dup = """    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT); // 409
    let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "SKU_ALREADY_EXISTS");"""
content = replace_in_func("test_create_variant_duplicate_sku", old_dup, new_dup, content)


with open("tests/operations.rs", "w") as f:
    f.write(content)

