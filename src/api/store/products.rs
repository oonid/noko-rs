use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{AppState, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreProductResponse {
    pub product: StoreProductView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreProductsResponse {
    pub products: Vec<StoreProductView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreProductView {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub variants: Vec<StoreVariantView>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreVariantView {
    pub id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub title: String,
    pub price: i64,
    pub currency_code: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct ProductVariantPriceRow {
    product_id: Uuid,
    product_title: String,
    product_description: String,
    product_status: String,
    product_created_at: OffsetDateTime,
    product_updated_at: OffsetDateTime,
    variant_id: Option<Uuid>,
    variant_sku: Option<String>,
    variant_title: Option<String>,
    variant_created_at: Option<OffsetDateTime>,
    variant_updated_at: Option<OffsetDateTime>,
    price_currency_code: Option<String>,
    price_amount: Option<i64>,
}

async fn list_products(
    State(state): State<AppState>,
) -> Result<Json<StoreProductsResponse>, AppError> {
    let rows = sqlx::query_as::<_, ProductVariantPriceRow>(
        r#"
        SELECT 
            p.id AS product_id,
            p.title AS product_title,
            p.description AS product_description,
            p.status AS product_status,
            p.created_at AS product_created_at,
            p.updated_at AS product_updated_at,
            v.id AS variant_id,
            v.sku AS variant_sku,
            v.title AS variant_title,
            v.created_at AS variant_created_at,
            v.updated_at AS variant_updated_at,
            vp.currency_code AS price_currency_code,
            vp.amount AS price_amount
        FROM products p
        JOIN product_variants v ON v.product_id = p.id
        JOIN variant_prices vp ON vp.variant_id = v.id AND vp.currency_code = 'IDR'
        WHERE p.status = 'active' AND v.active = true
        ORDER BY p.created_at DESC, p.id, v.created_at ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut products: Vec<StoreProductView> = Vec::new();
    for row in rows {
        if let Some(p) = products.iter_mut().find(|p| p.id == row.product_id) {
            if let (Some(vid), Some(sku), Some(title), Some(amount), Some(currency)) = (
                row.variant_id,
                row.variant_sku,
                row.variant_title,
                row.price_amount,
                row.price_currency_code,
            ) {
                p.variants.push(StoreVariantView {
                    id: vid,
                    product_id: row.product_id,
                    sku,
                    title,
                    price: amount,
                    currency_code: currency,
                    created_at: row.variant_created_at.unwrap_or(row.product_created_at),
                    updated_at: row.variant_updated_at.unwrap_or(row.product_updated_at),
                });
            }
        } else {
            let mut variants = Vec::new();
            if let (Some(vid), Some(sku), Some(title), Some(amount), Some(currency)) = (
                row.variant_id,
                row.variant_sku,
                row.variant_title,
                row.price_amount,
                row.price_currency_code,
            ) {
                variants.push(StoreVariantView {
                    id: vid,
                    product_id: row.product_id,
                    sku,
                    title,
                    price: amount,
                    currency_code: currency,
                    created_at: row.variant_created_at.unwrap_or(row.product_created_at),
                    updated_at: row.variant_updated_at.unwrap_or(row.product_updated_at),
                });
            }
            products.push(StoreProductView {
                id: row.product_id,
                title: row.product_title,
                description: row.product_description,
                status: row.product_status,
                variants,
                created_at: row.product_created_at,
                updated_at: row.product_updated_at,
            });
        }
    }

    Ok(Json(StoreProductsResponse { products }))
}

async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StoreProductResponse>, AppError> {
    let rows = sqlx::query_as::<_, ProductVariantPriceRow>(
        r#"
        SELECT 
            p.id AS product_id,
            p.title AS product_title,
            p.description AS product_description,
            p.status AS product_status,
            p.created_at AS product_created_at,
            p.updated_at AS product_updated_at,
            v.id AS variant_id,
            v.sku AS variant_sku,
            v.title AS variant_title,
            v.created_at AS variant_created_at,
            v.updated_at AS variant_updated_at,
            vp.currency_code AS price_currency_code,
            vp.amount AS price_amount
        FROM products p
        LEFT JOIN product_variants v ON v.product_id = p.id AND v.active = true
        LEFT JOIN variant_prices vp ON vp.variant_id = v.id AND vp.currency_code = 'IDR'
        WHERE p.id = $1
        ORDER BY v.created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    if rows.is_empty() {
        return Err(AppError::not_found("PRODUCT_NOT_FOUND"));
    }

    if rows[0].product_status != "active" {
        return Err(AppError::not_found("PRODUCT_NOT_FOUND"));
    }

    let first = &rows[0];
    let product_id = first.product_id;
    let product_title = first.product_title.clone();
    let product_description = first.product_description.clone();
    let product_status = first.product_status.clone();
    let product_created_at = first.product_created_at;
    let product_updated_at = first.product_updated_at;

    let mut variants = Vec::new();
    for row in rows {
        if let (Some(vid), Some(sku), Some(title), Some(amount), Some(currency)) = (
            row.variant_id,
            row.variant_sku,
            row.variant_title,
            row.price_amount,
            row.price_currency_code,
        ) {
            variants.push(StoreVariantView {
                id: vid,
                product_id: row.product_id,
                sku,
                title,
                price: amount,
                currency_code: currency,
                created_at: row.variant_created_at.unwrap_or(row.product_created_at),
                updated_at: row.variant_updated_at.unwrap_or(row.product_updated_at),
            });
        }
    }

    Ok(Json(StoreProductResponse {
        product: StoreProductView {
            id: product_id,
            title: product_title,
            description: product_description,
            status: product_status,
            variants,
            created_at: product_created_at,
            updated_at: product_updated_at,
        },
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", get(list_products))
        .route("/products/{id}", get(get_product))
}
