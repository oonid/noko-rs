use crate::cart::model::Cart;
use crate::cart::repository;
use crate::error::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn execute(conn: &mut PgConnection, customer_id: Uuid) -> Result<Cart, AppError> {
    let cart = repository::create_or_get_active_cart(conn, customer_id).await?;
    Ok(cart)
}
