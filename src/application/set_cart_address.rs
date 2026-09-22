use sqlx::PgConnection;
use uuid::Uuid;
use crate::cart::model::CartAddress;
use crate::cart::repository;
use crate::error::AppError;

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    customer_address_id: Uuid,
) -> Result<CartAddress, AppError> {
    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::not_found("cart_not_found"));
    }

    let addr = repository::set_cart_shipping_address(&mut *conn, cart_id, customer_address_id, customer_id).await?;
    match addr {
        Some(a) => Ok(a),
        None => Err(AppError::not_found("address_not_found")),
    }
}
