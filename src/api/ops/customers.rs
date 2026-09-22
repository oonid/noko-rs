use crate::app::AppState;
use crate::application::provision_registered_customer::{
    ProvisionRegisteredCustomerResult, provision_registered_customer,
};
use crate::auth::OpsCaller;
use crate::error::AppError;
use axum::{Json, extract::State};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProvisionRegisteredCustomerInput {
    pub auth_subject: String,
    pub display_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub first_name: String,
    pub last_name: String,
}

pub async fn handle_provision_customer(
    State(state): State<AppState>,
    _caller: OpsCaller,
    Json(payload): Json<ProvisionRegisteredCustomerInput>,
) -> Result<Json<ProvisionRegisteredCustomerResult>, AppError> {
    let result = provision_registered_customer(
        &state.pool,
        &payload.auth_subject,
        &payload.display_name,
        &payload.email,
        payload.phone.as_deref(),
        &payload.first_name,
        &payload.last_name,
    )
    .await?;

    Ok(Json(result))
}
