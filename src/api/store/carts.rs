use crate::api::extract::{Json, Path};
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
    extract::State,
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/carts/current", get(get_current_cart))
        .route("/carts", post(create_cart))
        .route("/carts/{id}/items", post(add_item))
        .route(
            "/carts/{id}/items/{item_id}",
            patch(update_item).delete(remove_item),
        )
        .route("/carts/{id}/shipping-address", post(set_address))
}

#[derive(Serialize)]
pub struct ActiveCartResponse {
    pub cart: Cart,
    pub items: Vec<CartItem>,
    pub shipping_address: Option<CartAddress>,
}

async fn get_current_cart(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
) -> Result<Json<ActiveCartResponse>, AppError> {
    let mut conn = state.pool.acquire().await?;
    // The requirement says: GET /store/carts/current (returns active Cart or 404)
    // create_or_get_active_cart creates if not exists, but we want to just get it. Let's just create it anyway as it's active. Wait, prompt says "returns active Cart or 404".
    // I should probably query it and return 404 if not found.
    let cart = sqlx::query_as::<_, Cart>(
        r#"
        SELECT id, customer_id, currency_code, status, created_at, updated_at, completed_at
        FROM carts
        WHERE customer_id = $1 AND status = 'active'
        "#,
    )
    .bind(ctx.customer_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("CART_NOT_FOUND"))?;

    let items = repository::get_cart_items(&mut conn, cart.id).await?;
    let shipping_address = repository::get_cart_address(&mut conn, cart.id, "shipping").await?;

    Ok(Json(ActiveCartResponse {
        cart,
        items,
        shipping_address,
    }))
}

async fn create_cart(
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
    Path(id): Path<Uuid>,
    Json(payload): Json<add_cart_item::AddCartItemInput>,
) -> Result<(StatusCode, Json<CartItem>), AppError> {
    let mut conn = state.pool.acquire().await?;

    let item = add_cart_item::execute(&mut conn, ctx.customer_id, id, payload).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn update_item(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<update_cart_item::UpdateCartItemInput>,
) -> Result<Json<CartItem>, AppError> {
    let mut conn = state.pool.acquire().await?;

    let item = update_cart_item::execute(&mut conn, ctx.customer_id, id, item_id, payload).await?;
    Ok(Json(item))
}

async fn remove_item(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.pool.acquire().await?;

    remove_cart_item::execute(&mut conn, ctx.customer_id, id, item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetAddressInput {
    pub customer_address_id: Uuid,
}

async fn set_address(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Path(id): Path<Uuid>,
    Json(payload): Json<SetAddressInput>,
) -> Result<Json<CartAddress>, AppError> {
    let mut conn = state.pool.acquire().await?;

    let addr =
        set_cart_address::execute(&mut conn, ctx.customer_id, id, payload.customer_address_id)
            .await?;
    Ok(Json(addr))
}
