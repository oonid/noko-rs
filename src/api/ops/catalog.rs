use crate::app::AppState;
use crate::application::create_sellable_variant::{
    CreateSellableVariantInput, create_sellable_variant,
};
use crate::auth::OpsCaller;
use crate::error::AppError;
use axum::Json;

pub async fn handle_create_variant(
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
