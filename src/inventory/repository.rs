use crate::error::AppError;
use crate::inventory::model::InventoryAvailability;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn availability_for_variant(
    pool: &PgPool,
    variant_id: Uuid,
) -> Result<InventoryAvailability, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT 
            i.id as inventory_item_id,
            l.id as inventory_level_id,
            l.stocked_quantity,
            l.reserved_quantity
        FROM inventory_items i
        JOIN inventory_levels l ON l.inventory_item_id = i.id
        JOIN inventory_locations loc ON loc.id = l.location_id
        WHERE i.variant_id = $1 AND loc.code = 'MAIN'
        "#,
        variant_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(InventoryAvailability {
            inventory_item_id: r.inventory_item_id,
            inventory_level_id: r.inventory_level_id,
            stocked_quantity: r.stocked_quantity,
            reserved_quantity: r.reserved_quantity,
            available_quantity: r.stocked_quantity - r.reserved_quantity,
        }),
        None => Err(AppError::not_found("INVENTORY_NOT_FOUND")),
    }
}

pub async fn adjust_inventory(
    pool: &PgPool,
    level_id: Uuid,
    delta: i64,
    reason: &str,
    note: Option<&str>,
    actor_id: Option<Uuid>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query!(
        r#"
        SELECT inventory_item_id, location_id, stocked_quantity 
        FROM inventory_levels 
        WHERE id = $1 
        FOR UPDATE
        "#,
        level_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let level = match row {
        Some(l) => l,
        None => return Err(AppError::not_found("INVENTORY_LEVEL_NOT_FOUND")),
    };

    let new_stocked = level.stocked_quantity + delta;
    if new_stocked < 0 {
        return Err(AppError::conflict("NEGATIVE_STOCK"));
    }

    sqlx::query!(
        r#"
        UPDATE inventory_levels 
        SET stocked_quantity = $1, updated_at = now()
        WHERE id = $2
        "#,
        new_stocked,
        level_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO inventory_adjustments 
        (inventory_item_id, location_id, delta, reason, note, actor_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        level.inventory_item_id,
        level.location_id,
        delta,
        reason,
        note,
        actor_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}
