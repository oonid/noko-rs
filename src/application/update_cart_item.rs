use crate::cart::model::CartItem;
use crate::cart::repository;
use crate::error::AppError;
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UpdateCartItemInput {
    pub quantity: i64,
}

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    item_id: Uuid,
    input: UpdateCartItemInput,
) -> Result<CartItem, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::bad_request(
            "INVALID_QUANTITY",
            "Quantity must be positive",
        ));
    }

    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::not_found("cart_not_found"));
    }

    // Get variant_id from item_id
    let item_record = sqlx::query!(
        "SELECT variant_id FROM cart_items WHERE id = $1 AND cart_id = $2",
        item_id,
        cart_id
    )
    .fetch_optional(&mut *conn)
    .await?;

    let variant_id = match item_record {
        Some(r) => r.variant_id,
        None => return Err(AppError::not_found("item_not_found")),
    };

    // Check inventory
    let avail = sqlx::query!(
        r#"
        SELECT l.stocked_quantity, l.reserved_quantity
        FROM inventory_items i
        JOIN inventory_levels l ON l.inventory_item_id = i.id
        JOIN inventory_locations loc ON loc.id = l.location_id
        WHERE i.variant_id = $1 AND loc.code = 'MAIN'
        "#,
        variant_id
    )
    .fetch_optional(&mut *conn)
    .await?;

    let avail_qty = avail
        .map(|a| a.stocked_quantity - a.reserved_quantity)
        .unwrap_or(0);

    if input.quantity > avail_qty {
        return Err(AppError::bad_request(
            "PRODUCT_NOT_AVAILABLE",
            "Not enough inventory available",
        ));
    }

    let item =
        repository::update_item_quantity(&mut *conn, cart_id, item_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::not_found("item_not_found")),
    }
}
