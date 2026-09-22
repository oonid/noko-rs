#![allow(clippy::explicit_auto_deref)]
use crate::cart::repository;
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn execute(
    pool: &PgPool,
    customer_id: Uuid,
    cart_id: Uuid,
    item_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let cart = repository::lock_cart(&mut *tx, cart_id, customer_id).await?;
    let cart = match cart {
        Some(c) => c,
        None => return Err(AppError::forbidden("CART_NOT_FOUND")),
    };
    if cart.status == "completed" {
        return Err(AppError::conflict("CART_COMPLETED"));
    }

    let removed = repository::remove_item(&mut *tx, cart_id, item_id).await?;
    if !removed {
        return Err(AppError::not_found("CART_ITEM_NOT_FOUND"));
    }

    tx.commit().await?;

    Ok(())
}
