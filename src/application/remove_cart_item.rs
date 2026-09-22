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
    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::not_found("cart_not_found"));
    }

    let removed = repository::remove_item(&mut *conn, cart_id, item_id).await?;
    if !removed {
        return Err(AppError::not_found("item_not_found"));
    }

    Ok(())
}
