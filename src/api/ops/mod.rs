use crate::app::AppState;
use crate::application::adjust_inventory::{AdjustInventoryInput, adjust_inventory};
use crate::application::create_sellable_variant::{
    CreateSellableVariantInput, create_sellable_variant,
};
use crate::auth::OpsCaller;
use crate::error::AppError;
use axum::{Json, Router, routing::post};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct AdjustInventoryRequest {
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub delta: i64,
    pub reason: String,
    pub note: Option<String>,
    pub initiator_external_ref: Option<String>, // Just for schema match, not actually stored or trusted
}

async fn handle_create_variant(
    _caller: OpsCaller,
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<CreateSellableVariantInput>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = create_sellable_variant(&state.pool, payload).await?;
    Ok(Json(serde_json::json!({
        "variant_id": result.0,
        "price_id": result.1,
        "inventory_item_id": result.2,
        "inventory_level_id": result.3
    })))
}

async fn handle_adjust_inventory(
    caller: OpsCaller,
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<AdjustInventoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.delta == 0 {
        return Err(AppError::validation("INVALID_INVENTORY_DELTA"));
    }
    let input = AdjustInventoryInput {
        inventory_item_id: payload.inventory_item_id,
        location_id: payload.location_id,
        delta: payload.delta,
        reason: payload.reason,
        note: payload.note,
        actor_id: Some(caller.actor.actor_id),
    };
    let level = adjust_inventory(&state.pool, input).await?;
    Ok(Json(serde_json::json!(level)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/catalog/variants", post(handle_create_variant))
        .route("/inventory/adjustments", post(handle_adjust_inventory))
}
