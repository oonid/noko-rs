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

    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::not_found("cart_not_found"));
    }

    let item =
        repository::add_item_to_cart(&mut *conn, cart_id, input.variant_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::not_found("variant_not_found")),
    }
}
