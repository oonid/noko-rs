use noko_rs::error::AppError;
use noko_rs::inventory::repository;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn test_availability_lookup_not_found(pool: PgPool) {
    let variant_id = Uuid::new_v4();
    let result = repository::availability_for_variant(&pool, variant_id).await;
    assert!(
        matches!(result, Err(AppError::NotFound { ref code, .. }) if code == "INVENTORY_NOT_FOUND")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inventory_adjust_concurrency(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, title) VALUES ($1, 'Prod')",
        product_id
    )
    .execute(&pool)
    .await
    .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku', 'V')",
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

    let location_id = sqlx::query!("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .id;

    let level_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)", level_id, item_id, location_id).execute(&pool).await.unwrap();

    let pool1 = pool.clone();
    let pool2 = pool.clone();

    let handle1 = tokio::spawn(async move {
        repository::adjust_inventory(&pool1, level_id, -3, "SALE", None, None)
            .await
            .unwrap();
    });
    let handle2 = tokio::spawn(async move {
        repository::adjust_inventory(&pool2, level_id, -4, "SALE", None, None)
            .await
            .unwrap();
    });

    handle1.await.unwrap();
    handle2.await.unwrap();

    let availability = repository::availability_for_variant(&pool, variant_id)
        .await
        .unwrap();
    assert_eq!(availability.stocked_quantity, 3);
    assert_eq!(availability.available_quantity, 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inventory_adjust_negative_stock(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO products (id, title) VALUES ($1, 'Prod')",
        product_id
    )
    .execute(&pool)
    .await
    .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku2', 'V')",
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

    let location_id = sqlx::query!("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .id;

    let level_id = Uuid::new_v4();
    sqlx::query!("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)", level_id, item_id, location_id).execute(&pool).await.unwrap();

    let err = repository::adjust_inventory(&pool, level_id, -15, "SALE", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Conflict { ref code, .. } if code == "NEGATIVE_STOCK"));
}
