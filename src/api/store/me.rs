use crate::app::AppState;
use crate::auth::AuthenticatedCustomer;
use crate::customer::{
    model::{CreateCustomerAddress, Customer, CustomerAddress},
    repository::{create_address, get_addresses, get_customer_by_id},
};
use crate::error::AppError;
use axum::http::StatusCode;
use axum::{
    Router,
    extract::{Json, State},
    routing::get,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(get_me)).route(
        "/me/addresses",
        get(get_my_addresses).post(create_my_address),
    )
}

async fn get_me(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
) -> Result<Json<Customer>, AppError> {
    let customer = get_customer_by_id(&state.pool, ctx.customer_id)
        .await?
        .ok_or_else(|| AppError::not_found("CUSTOMER_NOT_FOUND"))?;

    Ok(Json(customer))
}

async fn get_my_addresses(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
) -> Result<Json<Vec<CustomerAddress>>, AppError> {
    let addresses = get_addresses(&state.pool, ctx.customer_id).await?;
    Ok(Json(addresses))
}

async fn create_my_address(
    State(state): State<AppState>,
    AuthenticatedCustomer(ctx): AuthenticatedCustomer,
    Json(payload): Json<CreateCustomerAddress>,
) -> Result<(StatusCode, Json<CustomerAddress>), AppError> {
    let address = create_address(&state.pool, ctx.customer_id, payload).await?;
    Ok((StatusCode::CREATED, Json(address)))
}
