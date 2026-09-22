use sqlx::PgConnection;
use uuid::Uuid;
use crate::cart::model::CartItem;
use crate::cart::repository;
use crate::error::AppError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateCartItemInput {
    pub quantity: i64,
}

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    variant_id: Uuid,
    input: UpdateCartItemInput,
) -> Result<CartItem, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::bad_request("invalid_quantity", "Quantity must be positive")); // or 422? Error enum maps to 400 or 422
    }

    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::not_found("cart_not_found"));
    }

    let item = repository::update_item_quantity(&mut *conn, cart_id, variant_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::not_found("item_not_found")),
    }
}
