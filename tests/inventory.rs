use noko_rs::application::adjust_inventory::{AdjustInventoryInput, adjust_inventory};
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
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku', 'V')",
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

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let item_id1 = item_id;
    let item_id2 = item_id;

    let handle1 = tokio::spawn(async move {
        adjust_inventory(
            &pool1,
            AdjustInventoryInput {
                inventory_item_id: item_id1,
                location_id,
                delta: -3,
                reason: "SALE".to_string(),
                note: None,
                actor_id: None,
            },
        )
        .await
        .unwrap();
    });
    let handle2 = tokio::spawn(async move {
        adjust_inventory(
            &pool2,
            AdjustInventoryInput {
                inventory_item_id: item_id2,
                location_id,
                delta: -4,
                reason: "SALE".to_string(),
                note: None,
                actor_id: None,
            },
        )
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
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku2', 'V')",
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

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    let err = adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: -15,
            reason: "SALE".to_string(),
            note: None,
            actor_id: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Validation { ref code, .. } if code == "NEGATIVE_STOCK"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_truthful_shortage(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku3', 'V')",
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

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 8)")
        .bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: -4,
            reason: "SALE".to_string(),
            note: None,
            actor_id: None,
        },
    )
    .await
    .unwrap();

    let availability = repository::availability_for_variant(&pool, variant_id)
        .await
        .unwrap();
    assert_eq!(availability.stocked_quantity, 6);
    assert_eq!(availability.reserved_quantity, 8);
    assert_eq!(availability.available_quantity, -2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_db_constraints(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku_constraints', 'V')")
        .bind(variant_id).bind(product_id).execute(&pool).await.unwrap();

    let item_id1 = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id1)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    // InventoryItem.variant_id UNIQUE
    let item_id2 = Uuid::new_v4();
    let res = sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id2)
        .bind(variant_id)
        .execute(&pool)
        .await;
    assert!(res.is_err());

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id1 = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id1).bind(item_id1).bind(location_id).execute(&pool).await.unwrap();

    // InventoryLevel (item, location) UNIQUE
    let level_id2 = Uuid::new_v4();
    let res = sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id2).bind(item_id1).bind(location_id).execute(&pool).await;
    assert!(res.is_err());

    // stocked_quantity < 0 rejected by DB
    let res = sqlx::query("UPDATE inventory_levels SET stocked_quantity = -1 WHERE id = $1")
        .bind(level_id1)
        .execute(&pool)
        .await;
    assert!(res.is_err());

    // reserved_quantity < 0 rejected by DB
    let res = sqlx::query("UPDATE inventory_levels SET reserved_quantity = -1 WHERE id = $1")
        .bind(level_id1)
        .execute(&pool)
        .await;
    assert!(res.is_err());

    // reserved_quantity > stocked_quantity accepted by DB
    let res = sqlx::query("UPDATE inventory_levels SET reserved_quantity = 20 WHERE id = $1")
        .bind(level_id1)
        .execute(&pool)
        .await;
    assert!(res.is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_adjustment_audit(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku_audit', 'V')")
        .bind(variant_id).bind(product_id).execute(&pool).await.unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    let actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'audit_actor', 'Audit Actor')")
        .bind(actor_id).execute(&pool).await.unwrap();

    // Success audit
    adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: -3,
            reason: "SALE".to_string(),
            note: Some("Test note".to_string()),
            actor_id: Some(actor_id),
        },
    )
    .await
    .unwrap();

    let audit: (i64, String, Option<String>, Option<Uuid>) = sqlx::query_as("SELECT delta, reason, note, actor_id FROM inventory_adjustments WHERE inventory_item_id = $1 LIMIT 1")
        .bind(item_id).fetch_one(&pool).await.unwrap();

    assert_eq!(audit.0, -3);
    assert_eq!(audit.1, "SALE");
    assert_eq!(audit.2, Some("Test note".to_string()));
    assert_eq!(audit.3, Some(actor_id));

    // Failed adjustment
    let res = adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: -100, // will fail
            reason: "SALE".to_string(),
            note: None,
            actor_id: None,
        },
    )
    .await;
    assert!(res.is_err());

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM inventory_adjustments WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1); // no new audit row
}

#[sqlx::test(migrations = "./migrations")]
async fn test_overflow(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku_overflow', 'V')")
        .bind(variant_id).bind(product_id).execute(&pool).await.unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, $4, 0)")
        .bind(level_id).bind(item_id).bind(location_id).bind(i64::MAX).execute(&pool).await.unwrap();

    let err = adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: 1,
            reason: "OVERFLOW".to_string(),
            note: None,
            actor_id: None,
        },
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, AppError::Validation { ref code, .. } if code == "INVENTORY_QUANTITY_OVERFLOW")
    );

    let stocked: i64 =
        sqlx::query_scalar("SELECT stocked_quantity FROM inventory_levels WHERE id = $1")
            .bind(level_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stocked, i64::MAX);

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM inventory_adjustments WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_rollback(pool: PgPool) {
    let product_id = Uuid::new_v4();
    sqlx::query("INSERT INTO products (id, title) VALUES ($1, 'Prod')")
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO product_variants (id, product_id, sku, title) VALUES ($1, $2, 'sku_rollback', 'V')")
        .bind(variant_id).bind(product_id).execute(&pool).await.unwrap();

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_items (id, variant_id) VALUES ($1, $2)")
        .bind(item_id)
        .bind(variant_id)
        .execute(&pool)
        .await
        .unwrap();

    let location_id: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let level_id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_levels (id, inventory_item_id, location_id, stocked_quantity, reserved_quantity) VALUES ($1, $2, $3, 10, 0)")
        .bind(level_id).bind(item_id).bind(location_id).execute(&pool).await.unwrap();

    // Create a test-only constraint
    sqlx::query("ALTER TABLE inventory_adjustments ADD CONSTRAINT test_reject_reason CHECK (reason <> 'FORCE_ROLLBACK')")
        .execute(&pool).await.unwrap();

    let res = adjust_inventory(
        &pool,
        AdjustInventoryInput {
            inventory_item_id: item_id,
            location_id,
            delta: -3,
            reason: "FORCE_ROLLBACK".to_string(),
            note: None,
            actor_id: None,
        },
    )
    .await;

    assert!(res.is_err());

    // Should rollback, stocked should remain 10
    let stocked: i64 =
        sqlx::query_scalar("SELECT stocked_quantity FROM inventory_levels WHERE id = $1")
            .bind(level_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stocked, 10);

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM inventory_adjustments WHERE inventory_item_id = $1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}
