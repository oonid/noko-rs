#![allow(clippy::explicit_auto_deref)]
use crate::cart::model::CartItem;
use crate::cart::repository;
use crate::error::AppError;
use serde::Deserialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UpdateCartItemInput {
    pub quantity: i64,
}

pub async fn execute(
    pool: &PgPool,
    customer_id: Uuid,
    cart_id: Uuid,
    item_id: Uuid,
    input: UpdateCartItemInput,
) -> Result<CartItem, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::validation("INVALID_QUANTITY"));
    }

    let mut tx = pool.begin().await?;

    let cart = repository::lock_cart(&mut *tx, cart_id, customer_id).await?;
    let cart = match cart {
        Some(c) => c,
        None => return Err(AppError::forbidden("CART_NOT_FOUND")),
    };
    if cart.status == "completed" {
        return Err(AppError::conflict("CART_COMPLETED"));
    }

    // Get variant_id from item_id
    let item_record =
        sqlx::query("SELECT variant_id FROM cart_items WHERE id = $1 AND cart_id = $2")
            .bind(item_id)
            .bind(cart_id)
            .fetch_optional(&mut *tx)
            .await?;

    let variant_id = match item_record {
        Some(r) => r.try_get::<Uuid, _>("variant_id")?,
        None => return Err(AppError::not_found("CART_ITEM_NOT_FOUND")),
    };

    // Check inventory
    let avail = sqlx::query(
        r#"
        SELECT l.stocked_quantity, l.reserved_quantity
        FROM inventory_items i
        JOIN inventory_levels l ON l.inventory_item_id = i.id
        JOIN inventory_locations loc ON loc.id = l.location_id
        WHERE i.variant_id = $1 AND loc.code = 'MAIN' AND loc.active = true
        "#,
    )
    .bind(variant_id)
    .fetch_optional(&mut *tx)
    .await?;

    let avail_qty = if let Some(a) = avail {
        let sq: i64 = a.try_get("stocked_quantity")?;
        let rq: i64 = a.try_get("reserved_quantity")?;
        sq - rq
    } else {
        0
    };

    if input.quantity > avail_qty {
        return Err(AppError::bad_request(
            "PRODUCT_NOT_AVAILABLE",
            "Not enough inventory available",
        ));
    }

    let item = repository::update_item_quantity(&mut *tx, cart_id, item_id, input.quantity).await?;
    let item = match item {
        Some(i) => i,
        None => return Err(AppError::not_found("CART_ITEM_NOT_FOUND")),
    };

    tx.commit().await?;

    Ok(item)
}
