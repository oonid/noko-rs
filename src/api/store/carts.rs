use crate::app::AppState;
use crate::application::{
    add_cart_item, create_or_get_active_cart, remove_cart_item, set_cart_address, update_cart_item,
};
use crate::auth::AuthenticatedCustomer;
use crate::cart::model::{Cart, CartAddress, CartItem};
use crate::cart::repository;
use crate::error::AppError;
use axum::http::StatusCode;
use axum::{
    Router,
    extract::{Json, Path, State},
    routing::{get, patch, post, put},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/carts/active", get(get_active_cart))
        .route("/carts/active/items", post(add_item))
        .route(
            "/carts/active/items/{variant_id}",
            patch(update_item).delete(remove_item),
        )
        .route("/carts/active/address", put(set_address))
}

#[derive(Serialize)]
pub struct ActiveCartResponse {
    pub cart: Cart,
    pub items: Vec<CartItem>,
    pub shipping_address: Option<CartAddress>,
}

async fn get_active_cart(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
) -> Result<Json<ActiveCartResponse>, AppError> {
    let mut conn = state.pool.acquire().await?;
    let cart = create_or_get_active_cart::execute(&mut conn, ctx.customer_id).await?;
    let items = repository::get_cart_items(&mut conn, cart.id).await?;
    let shipping_address = repository::get_cart_address(&mut conn, cart.id, "shipping").await?;

    Ok(Json(ActiveCartResponse {
        cart,
        items,
        shipping_address,
    }))
}

async fn add_item(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Json(payload): Json<add_cart_item::AddCartItemInput>,
) -> Result<(StatusCode, Json<CartItem>), AppError> {
    let mut conn = state.pool.acquire().await?;
    let cart = create_or_get_active_cart::execute(&mut conn, ctx.customer_id).await?;
    let item = add_cart_item::execute(&mut conn, ctx.customer_id, cart.id, payload).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn update_item(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Path(variant_id): Path<Uuid>,
    Json(payload): Json<update_cart_item::UpdateCartItemInput>,
) -> Result<Json<CartItem>, AppError> {
    let mut conn = state.pool.acquire().await?;
    let cart = create_or_get_active_cart::execute(&mut conn, ctx.customer_id).await?;
    let item =
        update_cart_item::execute(&mut conn, ctx.customer_id, cart.id, variant_id, payload).await?;
    Ok(Json(item))
}

async fn remove_item(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Path(variant_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.acquire().await?;
    let cart = create_or_get_active_cart::execute(&mut conn, ctx.customer_id).await?;
    remove_cart_item::execute(&mut conn, ctx.customer_id, cart.id, variant_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetAddressInput {
    pub customer_address_id: Uuid,
}

async fn set_address(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Json(payload): Json<SetAddressInput>,
) -> Result<Json<CartAddress>, AppError> {
    let mut conn = state.pool.acquire().await?;
    let cart = create_or_get_active_cart::execute(&mut conn, ctx.customer_id).await?;
    let addr = set_cart_address::execute(
        &mut conn,
        ctx.customer_id,
        cart.id,
        payload.customer_address_id,
    )
    .await?;
    Ok(Json(addr))
}
