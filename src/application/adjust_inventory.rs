use crate::error::AppError;
use crate::inventory::repository;
use sqlx::PgPool;
use uuid::Uuid;

pub struct AdjustInventoryInput {
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub delta: i64,
    pub reason: String,
    pub note: Option<String>,
    pub actor_id: Option<Uuid>,
}

pub async fn adjust_inventory(pool: &PgPool, input: AdjustInventoryInput) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let level = repository::lock_level(&mut tx, input.inventory_item_id, input.location_id).await?;

    let new_stocked = level
        .stocked_quantity
        .checked_add(input.delta)
        .ok_or_else(|| AppError::validation("INVENTORY_QUANTITY_OVERFLOW"))?;

    if new_stocked < 0 {
        return Err(AppError::validation("NEGATIVE_STOCK"));
    }

    repository::set_stocked(&mut tx, level.id, new_stocked).await?;

    repository::insert_adjustment(
        &mut tx,
        input.inventory_item_id,
        input.location_id,
        input.delta,
        &input.reason,
        input.note.as_deref(),
        input.actor_id,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}
