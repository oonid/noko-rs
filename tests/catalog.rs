use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use noko_rs::{
    AppState, app::build_router, catalog::repository as catalog_repo, config::Config,
    pricing::repository as pricing_repo,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn sku_is_unique(pool: PgPool) -> sqlx::Result<()> {
    let product_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Test Product', 'Description', 'active')
        "#,
    )
    .bind(product_id)
    .execute(&pool)
    .await?;

    let variant1_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TEST-SKU-1', 'Variant 1', true)
        "#,
    )
    .bind(variant1_id)
    .bind(product_id)
    .execute(&pool)
    .await?;

    let variant2_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TEST-SKU-1', 'Variant 2', true)
        "#,
    )
    .bind(variant2_id)
    .bind(product_id)
    .execute(&pool)
    .await;

    assert!(result.is_err(), "Duplicate SKU should be rejected");
    let err = result.unwrap_err();
    let db_err = err.as_database_error().expect("must be a database error");
    assert_eq!(db_err.code().as_deref(), Some("23505")); // unique_violation

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn variant_price_rejects_non_idr_currency(pool: PgPool) -> sqlx::Result<()> {
    let product_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Product USD', 'Description', 'active')
        "#,
    )
    .bind(product_id)
    .execute(&pool)
    .await?;

    let variant_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'SKU-USD', 'Variant USD', true)
        "#,
    )
    .bind(variant_id)
    .bind(product_id)
    .execute(&pool)
    .await?;

    let result = sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'USD', 1000)
        "#,
    )
    .bind(variant_id)
    .execute(&pool)
    .await;

    assert!(result.is_err(), "Currency other than IDR must be rejected");
    let err = result.unwrap_err();
    let db_err = err.as_database_error().expect("must be database error");
    assert_eq!(db_err.code().as_deref(), Some("23514")); // check_violation

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn variant_price_rejects_negative_amount(pool: PgPool) -> sqlx::Result<()> {
    let product_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Product Neg', 'Description', 'active')
        "#,
    )
    .bind(product_id)
    .execute(&pool)
    .await?;

    let variant_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'SKU-NEG', 'Variant Neg', true)
        "#,
    )
    .bind(variant_id)
    .bind(product_id)
    .execute(&pool)
    .await?;

    let result = sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', -500)
        "#,
    )
    .bind(variant_id)
    .execute(&pool)
    .await;

    assert!(result.is_err(), "Negative amount must be rejected");
    let err = result.unwrap_err();
    let db_err = err.as_database_error().expect("must be database error");
    assert_eq!(db_err.code().as_deref(), Some("23514")); // check_violation

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn get_active_variant_and_idr_price_repositories(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.acquire().await?;

    let product_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Repo Product', 'Description', 'active')
        "#,
    )
    .bind(product_id)
    .execute(&mut *conn)
    .await?;

    let active_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'ACTIVE-VAR', 'Active Variant', true)
        "#,
    )
    .bind(active_var_id)
    .bind(product_id)
    .execute(&mut *conn)
    .await?;

    let inactive_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'INACTIVE-VAR', 'Inactive Variant', false)
        "#,
    )
    .bind(inactive_var_id)
    .bind(product_id)
    .execute(&mut *conn)
    .await?;

    let price_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO variant_prices (id, variant_id, currency_code, amount)
        VALUES ($1, $2, 'IDR', 45000)
        "#,
    )
    .bind(price_id)
    .bind(active_var_id)
    .execute(&mut *conn)
    .await?;

    // 1. Fetch active variant
    let variant = catalog_repo::get_active_variant(&mut conn, active_var_id).await?;
    assert_eq!(variant.id, active_var_id);
    assert_eq!(variant.sku, "ACTIVE-VAR");
    assert!(variant.active);

    // 2. Inactive variant returns not found
    let inactive_res = catalog_repo::get_active_variant(&mut conn, inactive_var_id).await;
    assert!(
        matches!(inactive_res, Err(noko_rs::error::AppError::NotFound { ref code, .. }) if code == "VARIANT_NOT_FOUND"),
        "Inactive variant should return VARIANT_NOT_FOUND error"
    );

    // 3. Nonexistent variant returns not found
    let non_existent = catalog_repo::get_active_variant(&mut conn, Uuid::new_v4()).await;
    assert!(
        matches!(non_existent, Err(noko_rs::error::AppError::NotFound { ref code, .. }) if code == "VARIANT_NOT_FOUND"),
        "Nonexistent variant should return VARIANT_NOT_FOUND error"
    );

    // 3a. Draft product + active variant returns not found
    let draft_prod_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Draft Product', 'Description', 'draft')
        "#,
    )
    .bind(draft_prod_id)
    .execute(&mut *conn)
    .await?;

    let draft_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'DRAFT-VAR', 'Draft Variant', true)
        "#,
    )
    .bind(draft_var_id)
    .bind(draft_prod_id)
    .execute(&mut *conn)
    .await?;

    let draft_res = catalog_repo::get_active_variant(&mut conn, draft_var_id).await;
    assert!(
        matches!(draft_res, Err(noko_rs::error::AppError::NotFound { ref code, .. }) if code == "VARIANT_NOT_FOUND"),
        "Draft product variant should return VARIANT_NOT_FOUND error"
    );

    // 3b. Archived product + active variant returns not found
    let arch_prod_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Archived Product', 'Description', 'archived')
        "#,
    )
    .bind(arch_prod_id)
    .execute(&mut *conn)
    .await?;

    let arch_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'ARCH-VAR', 'Arch Variant', true)
        "#,
    )
    .bind(arch_var_id)
    .bind(arch_prod_id)
    .execute(&mut *conn)
    .await?;

    let arch_res = catalog_repo::get_active_variant(&mut conn, arch_var_id).await;
    assert!(
        matches!(arch_res, Err(noko_rs::error::AppError::NotFound { ref code, .. }) if code == "VARIANT_NOT_FOUND"),
        "Archived product variant should return VARIANT_NOT_FOUND error"
    );

    // 4. Fetch IDR price
    let price = pricing_repo::get_idr_price(&mut conn, active_var_id).await?;
    assert_eq!(price.id, price_id);
    assert_eq!(price.variant_id, active_var_id);
    assert_eq!(price.currency_code, "IDR");
    assert_eq!(price.amount, 45000);

    // 5. Price for variant without price returns not found
    let no_price = pricing_repo::get_idr_price(&mut conn, inactive_var_id).await;
    assert!(
        matches!(no_price, Err(noko_rs::error::AppError::NotFound { ref code, .. }) if code == "PRICE_NOT_FOUND"),
        "Missing price should return PRICE_NOT_FOUND error"
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn store_products_http_reads(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(Config {
        database_url: "postgres://noko_test:noko_test@127.0.0.1:5432/noko_test".to_string(),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev_header".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 2,
    });
    let state = AppState {
        pool: pool.clone(),
        config,
    };
    let app = build_router(state);

    // Seed data:
    // 1. Active product with 1 active variant and IDR price
    let active_prod_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Active T-Shirt', 'A great t-shirt', 'active')
        "#,
    )
    .bind(active_prod_id)
    .execute(&pool)
    .await?;

    let active_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TSHIRT-M', 'Medium', true)
        "#,
    )
    .bind(active_var_id)
    .bind(active_prod_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', 125000)
        "#,
    )
    .bind(active_var_id)
    .execute(&pool)
    .await?;

    let active_var_2_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TSHIRT-L', 'Large', true)
        "#,
    )
    .bind(active_var_2_id)
    .bind(active_prod_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', 130000)
        "#,
    )
    .bind(active_var_2_id)
    .execute(&pool)
    .await?;

    // Inactive variant under the active product (should NOT appear)
    let inactive_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TSHIRT-XL', 'Extra Large', false)
        "#,
    )
    .bind(inactive_var_id)
    .bind(active_prod_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', 135000)
        "#,
    )
    .bind(inactive_var_id)
    .execute(&pool)
    .await?;

    // Active variant without price under the active product (should NOT appear)
    let no_price_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'TSHIRT-S', 'Small', true)
        "#,
    )
    .bind(no_price_var_id)
    .bind(active_prod_id)
    .execute(&pool)
    .await?;

    // 2. Inactive product (draft) with active variant & price (must NOT be listed)
    let draft_prod_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO products (id, title, description, status)
        VALUES ($1, 'Draft Hoodie', 'Draft description', 'draft')
        "#,
    )
    .bind(draft_prod_id)
    .execute(&pool)
    .await?;

    let draft_var_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO product_variants (id, product_id, sku, title, active)
        VALUES ($1, $2, 'HOODIE-M', 'Medium Hoodie', true)
        "#,
    )
    .bind(draft_var_id)
    .bind(draft_prod_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', 250000)
        "#,
    )
    .bind(draft_var_id)
    .execute(&pool)
    .await?;

    // --- Assert 1: GET /store/products lists only active products with active variants and IDR prices ---
    let req = Request::builder()
        .uri("/store/products")
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    let products = json["products"].as_array().expect("products array");
    assert_eq!(
        products.len(),
        1,
        "Only the active product should be listed"
    );
    assert_eq!(products[0]["id"], active_prod_id.to_string());
    assert_eq!(products[0]["title"], "Active T-Shirt");

    let variants = products[0]["variants"].as_array().expect("variants array");
    assert_eq!(
        variants.len(),
        2,
        "Both active variants with IDR price should be included"
    );
    assert_eq!(variants[0]["id"], active_var_id.to_string());
    assert_eq!(variants[1]["id"], active_var_2_id.to_string());

    // --- Assert 2: Variant price is represented as integer IDR ---
    let price = &variants[0]["price"];
    assert!(price.is_i64(), "Variant price must be an integer IDR");
    assert_eq!(price.as_i64(), Some(125000));
    assert_eq!(variants[0]["currency_code"], "IDR");
    assert_eq!(variants[1]["price"], 130000);

    // --- Assert 3: GET /store/products/:id for active product returns 200 ---
    let req = Request::builder()
        .uri(format!("/store/products/{}", active_prod_id))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
    let prod_json: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    assert_eq!(prod_json["product"]["id"], active_prod_id.to_string());
    let prod_variants = prod_json["product"]["variants"]
        .as_array()
        .expect("variants array");
    assert_eq!(prod_variants.len(), 2);
    assert_eq!(prod_variants[0]["price"], 125000);
    assert_eq!(prod_variants[1]["price"], 130000);

    // --- Assert 4: Unknown product returns 404 ---
    let unknown_id = Uuid::new_v4();
    let req = Request::builder()
        .uri(format!("/store/products/{}", unknown_id))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // --- Assert 5: Inactive (draft) product returns 404 ---
    let req = Request::builder()
        .uri(format!("/store/products/{}", draft_prod_id))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // --- Assert 6: No mutation endpoint is exposed under /store/products ---
    let req = Request::builder()
        .method("POST")
        .uri("/store/products")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title":"Malicious Product"}"#))?;
    let resp = app.clone().oneshot(req).await?;
    assert!(
        resp.status() == StatusCode::METHOD_NOT_ALLOWED || resp.status() == StatusCode::NOT_FOUND,
        "POST /store/products must be rejected with 405 or 404, got {}",
        resp.status()
    );

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/store/products/{}", active_prod_id))
        .body(Body::empty())?;
    let resp = app.oneshot(req).await?;
    assert!(
        resp.status() == StatusCode::METHOD_NOT_ALLOWED || resp.status() == StatusCode::NOT_FOUND,
        "DELETE /store/products/:id must be rejected with 405 or 404, got {}",
        resp.status()
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_public_catalog_no_auth(pool: sqlx::PgPool) {
    let config = std::sync::Arc::new(noko_rs::config::Config {
        database_url: std::env::var("DATABASE_URL").unwrap(),
        bind_addr: "127.0.0.1:0".to_string(),
        auth_mode: "dev_header".to_string(),
        nocodb_service_token: None,
        nocodb_service_actor_id: None,
        db_tx_max_retries: 2,
    });
    let app = noko_rs::app::build_router(noko_rs::app::AppState {
        pool: pool.clone(),
        config,
    });

    let req = axum::http::Request::builder()
        .uri("/store/products")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = tower::ServiceExt::oneshot(app, req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
}
