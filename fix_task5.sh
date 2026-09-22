#!/bin/bash
set -e
# Create src/api/ops/mod.rs
mkdir -p src/api/ops

cat << 'INNER' > src/api/ops/mod.rs
use axum::{routing::post, Json, Router};
use uuid::Uuid;
use crate::app::AppState;
use crate::auth::OpsCaller;
use crate::error::AppError;
use crate::application::create_sellable_variant::{create_sellable_variant, CreateSellableVariantInput};
use crate::application::adjust_inventory::{adjust_inventory, AdjustInventoryInput};

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
    caller: OpsCaller,
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
INNER

# Add ops to api
sed -i 's/pub mod store;/pub mod store;\npub mod ops;/g' src/api/mod.rs
sed -i 's/Router::new().nest("\/store", store::router())/Router::new().nest("\/store", store::router()).nest("\/ops", ops::router())/g' src/api/mod.rs

# Create create_sellable_variant.rs
cat << 'INNER' > src/application/create_sellable_variant.rs
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct CreateSellableVariantInput {
    pub product_id: Uuid,
    pub sku: String,
    pub title: String,
    pub amount: rust_decimal::Decimal,
}

pub async fn create_sellable_variant(
    pool: &PgPool,
    input: CreateSellableVariantInput,
) -> Result<(Uuid, Uuid, Uuid, Uuid), AppError> {
    use rust_decimal::Decimal;
    use std::str::FromStr;
    if input.amount < Decimal::from_str("0").unwrap() {
        return Err(AppError::validation("INVALID_PRICE_AMOUNT"));
    }

    let mut tx = pool.begin().await?;

    let product_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM products WHERE id = $1)")
        .bind(input.product_id)
        .fetch_one(&mut *tx)
        .await?;

    if !product_exists {
        return Err(AppError::not_found("Product not found"));
    }

    let variant_id = crate::catalog::repository::create_variant(
        &mut tx,
        input.product_id,
        &input.sku,
        &input.title,
    ).await.map_err(|e| {
        if e.to_string().contains("23505") {
            AppError::conflict("SKU_ALREADY_EXISTS")
        } else {
            e
        }
    })?;

    let price_id = crate::pricing::repository::create_idr_price(
        &mut tx,
        variant_id,
        input.amount,
    ).await?;

    let item_id = crate::inventory::repository::create_item(
        &mut tx,
        variant_id,
        &input.sku,
        true,
        true,
    ).await?;

    let location_id = crate::inventory::repository::find_main_location(&mut tx).await?;

    let level_id = crate::inventory::repository::create_level(
        &mut tx,
        item_id,
        location_id,
        0,
        0,
    ).await?;

    tx.commit().await?;

    Ok((variant_id, price_id, item_id, level_id))
}
INNER

# Add to application/mod.rs
echo "pub mod create_sellable_variant;" >> src/application/mod.rs

