use crate::error::AppError;
use crate::inventory::model::{InventoryAvailability, InventoryLevel};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct AvailabilityRow {
    inventory_item_id: Uuid,
    inventory_level_id: Uuid,
    stocked_quantity: i64,
    reserved_quantity: i64,
}

pub async fn availability_for_variant(
    pool: &PgPool,
    variant_id: Uuid,
) -> Result<InventoryAvailability, AppError> {
    let row: Option<AvailabilityRow> = sqlx::query_as(
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
    )
    .bind(variant_id)
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

pub async fn lock_level(
    tx: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    location_id: Uuid,
) -> Result<InventoryLevel, AppError> {
    let level: Option<InventoryLevel> = sqlx::query_as(
        r#"
        SELECT id, inventory_item_id, location_id, stocked_quantity, reserved_quantity, created_at, updated_at
        FROM inventory_levels 
        WHERE inventory_item_id = $1 AND location_id = $2
        FOR UPDATE
        "#
    )
    .bind(inventory_item_id)
    .bind(location_id)
    .fetch_optional(&mut **tx)
    .await?;

    level.ok_or_else(|| AppError::not_found("INVENTORY_LEVEL_NOT_FOUND"))
}

pub async fn set_stocked(
    tx: &mut Transaction<'_, Postgres>,
    level_id: Uuid,
    new_stocked: i64,
) -> Result<InventoryLevel, AppError> {
    let level: InventoryLevel = sqlx::query_as(
        r#"
        UPDATE inventory_levels 
        SET stocked_quantity = $1, updated_at = now()
        WHERE id = $2
        RETURNING id, inventory_item_id, location_id, stocked_quantity, reserved_quantity, created_at, updated_at
        "#,
    )
    .bind(new_stocked)
    .bind(level_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(level)
}

pub async fn insert_adjustment(
    tx: &mut Transaction<'_, Postgres>,
    inventory_item_id: Uuid,
    location_id: Uuid,
    delta: i64,
    reason: &str,
    note: Option<&str>,
    actor_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO inventory_adjustments 
        (inventory_item_id, location_id, delta, reason, note, actor_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(inventory_item_id)
    .bind(location_id)
    .bind(delta)
    .bind(reason)
    .bind(note)
    .bind(actor_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

use sqlx::PgConnection;
pub async fn create_item(
    conn: &mut PgConnection,
    variant_id: Uuid,
    _sku: &str,
    _requires_shipping: bool,
    _tracked: bool,
) -> Result<Uuid, AppError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO inventory_items (variant_id)
        VALUES ($1)
        RETURNING id
        "#,
    )
    .bind(variant_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

pub async fn find_main_location(conn: &mut PgConnection) -> Result<Uuid, AppError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM inventory_locations LIMIT 1
        "#,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("LOCATION_NOT_FOUND"))?; // fallback if no locations exist in test? Or maybe it errors?
    Ok(id)
}

pub async fn create_level(
    conn: &mut PgConnection,
    item_id: Uuid,
    location_id: Uuid,
    stocked: i64,
    reserved: i64,
) -> Result<Uuid, AppError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO inventory_levels (inventory_item_id, location_id, stocked_quantity, reserved_quantity)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#
    )
    .bind(item_id)
    .bind(location_id)
    .bind(stocked)
    .bind(reserved)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}
