use crate::cart::model::CartItem;
use crate::cart::repository;
use crate::error::AppError;
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AddCartItemInput {
    pub variant_id: Uuid,
    pub quantity: i64,
}

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    input: AddCartItemInput,
) -> Result<CartItem, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::bad_request(
            "invalid_quantity",
            "Quantity must be positive",
        ));
    }

    let cart = repository::lock_cart(&mut *conn, cart_id, customer_id).await?;
    let cart = match cart {
        Some(c) => c,
        None => return Err(AppError::forbidden("CART_NOT_FOUND")),
    };
    if cart.status == "completed" {
        return Err(AppError::conflict("CART_COMPLETED"));
    }

    // Check inventory
    // Check inventory
    use sqlx::Row;
    let avail = sqlx::query(
        r#"
        SELECT l.stocked_quantity, l.reserved_quantity
        FROM inventory_items i
        JOIN inventory_levels l ON l.inventory_item_id = i.id
        JOIN inventory_locations loc ON loc.id = l.location_id
        WHERE i.variant_id = $1 AND loc.code = 'MAIN'
        "#,
    )
    .bind(input.variant_id)
    .fetch_optional(&mut *conn)
    .await?;

    let avail_qty = avail
        .map(|a| {
            let sq: i64 = a.try_get("stocked_quantity").unwrap_or(0);
            let rq: i64 = a.try_get("reserved_quantity").unwrap_or(0);
            sq - rq
        })
        .unwrap_or(0);

    // Check existing item in cart
    let existing = sqlx::query(
        r#"
        SELECT quantity FROM cart_items WHERE cart_id = $1 AND variant_id = $2
        "#,
    )
    .bind(cart_id)
    .bind(input.variant_id)
    .fetch_optional(&mut *conn)
    .await?;

    let existing_qty = existing
        .map(|e| e.try_get::<i64, _>("quantity").unwrap_or(0))
        .unwrap_or(0);

    if existing_qty + input.quantity > avail_qty {
        return Err(AppError::bad_request(
            "PRODUCT_NOT_AVAILABLE",
            "Not enough inventory available",
        ));
    }

    let item =
        repository::add_item_to_cart(&mut *conn, cart_id, input.variant_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::not_found("variant_not_found")),
    }
}
