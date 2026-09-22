#![allow(clippy::explicit_auto_deref)]
use crate::cart::model::CartAddress;
use crate::cart::repository;
use crate::customer::repository::get_address_for_customer;
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn execute(
    pool: &PgPool,
    customer_id: Uuid,
    cart_id: Uuid,
    customer_address_id: Uuid,
) -> Result<CartAddress, AppError> {
    let mut tx = pool.begin().await?;

    let cart = repository::lock_cart(&mut *tx, cart_id, customer_id).await?;
    let cart = match cart {
        Some(c) => c,
        None => return Err(AppError::forbidden("CART_NOT_FOUND")),
    };
    if cart.status == "completed" {
        return Err(AppError::conflict("CART_COMPLETED"));
    }

    let customer_addr =
        get_address_for_customer(&mut *tx, customer_id, customer_address_id).await?;
    let customer_addr = match customer_addr {
        Some(a) => a,
        None => return Err(AppError::not_found("CUSTOMER_ADDRESS_NOT_FOUND")),
    };

    let addr = repository::set_cart_shipping_address(&mut *tx, cart_id, &customer_addr).await?;

    tx.commit().await?;

    Ok(addr)
}
