use crate::cart::repository;
use crate::error::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    item_id: Uuid,
) -> Result<(), AppError> {
    let cart = repository::lock_cart(&mut *conn, cart_id, customer_id).await?;
    let cart = match cart {
        Some(c) => c,
        None => return Err(AppError::forbidden("CART_NOT_FOUND")),
    };
    if cart.status == "completed" {
        return Err(AppError::conflict("CART_COMPLETED"));
    }

    let removed = repository::remove_item(&mut *conn, cart_id, item_id).await?;
    if !removed {
        return Err(AppError::not_found("item_not_found"));
    }

    Ok(())
}
