import os

with open("src/application/mod.rs", "a") as f:
    f.write("pub mod create_or_get_active_cart;\n")
    f.write("pub mod add_cart_item;\n")
    f.write("pub mod update_cart_item;\n")
    f.write("pub mod remove_cart_item;\n")
    f.write("pub mod set_cart_address;\n")

with open("src/application/create_or_get_active_cart.rs", "w") as f:
    f.write("""use sqlx::PgConnection;
use uuid::Uuid;
use crate::cart::model::Cart;
use crate::cart::repository;
use crate::error::AppError;

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
) -> Result<Cart, AppError> {
    let cart = repository::create_or_get_active_cart(conn, customer_id).await?;
    Ok(cart)
}
""")

with open("src/application/add_cart_item.rs", "w") as f:
    f.write("""use sqlx::PgConnection;
use uuid::Uuid;
use crate::cart::model::CartItem;
use crate::cart::repository;
use crate::error::AppError;
use serde::Deserialize;

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
        return Err(AppError::BadRequest("Quantity must be positive".to_string()));
    }

    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::NotFound("Cart not found or not active".to_string()));
    }

    let item = repository::add_item_to_cart(&mut *conn, cart_id, input.variant_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::NotFound("Variant not found, not active, or no IDR pricing".to_string())),
    }
}
""")

with open("src/application/update_cart_item.rs", "w") as f:
    f.write("""use sqlx::PgConnection;
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
        return Err(AppError::BadRequest("Quantity must be positive".to_string())); // or 422? Error enum maps to 400 or 422
    }

    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::NotFound("Cart not found or not active".to_string()));
    }

    let item = repository::update_item_quantity(&mut *conn, cart_id, variant_id, input.quantity).await?;
    match item {
        Some(i) => Ok(i),
        None => Err(AppError::NotFound("Item not found in cart".to_string())),
    }
}
""")

with open("src/application/remove_cart_item.rs", "w") as f:
    f.write("""use sqlx::PgConnection;
use uuid::Uuid;
use crate::cart::repository;
use crate::error::AppError;

pub async fn execute(
    conn: &mut PgConnection,
    customer_id: Uuid,
    cart_id: Uuid,
    variant_id: Uuid,
) -> Result<(), AppError> {
    let cart = repository::lock_active_cart(&mut *conn, cart_id, customer_id).await?;
    if cart.is_none() {
        return Err(AppError::NotFound("Cart not found or not active".to_string()));
    }

    let removed = repository::remove_item(&mut *conn, cart_id, variant_id).await?;
    if !removed {
        return Err(AppError::NotFound("Item not found in cart".to_string()));
    }
    
    Ok(())
}
""")

with open("src/application/set_cart_address.rs", "w") as f:
    f.write("""use sqlx::PgConnection;
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
        return Err(AppError::NotFound("Cart not found or not active".to_string()));
    }

    let addr = repository::set_cart_shipping_address(&mut *conn, cart_id, customer_address_id, customer_id).await?;
    match addr {
        Some(a) => Ok(a),
        None => Err(AppError::NotFound("Customer address not found".to_string())),
    }
}
""")

