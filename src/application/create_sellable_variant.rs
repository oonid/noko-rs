use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct CreateSellableVariantInput {
    pub product_id: Uuid,
    pub sku: String,
    pub title: String,
    pub amount: i64,
}

pub async fn create_sellable_variant(
    pool: &PgPool,
    input: CreateSellableVariantInput,
) -> Result<(Uuid, Uuid, Uuid, Uuid), AppError> {
    if input.amount < 0 {
        return Err(AppError::validation("INVALID_PRICE_AMOUNT"));
    }

    let mut tx = pool.begin().await?;

    let product_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM products WHERE id = $1)")
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
    )
    .await
                .map_err(|e| match &e {
        AppError::Database(db_err) if db_err.as_database_error().and_then(|err| err.code()).as_deref() == Some("23505") => {
            AppError::conflict("SKU_ALREADY_EXISTS")
        }
        _ => e,
    })?;

    let price_id =
        crate::pricing::repository::create_idr_price(&mut tx, variant_id, input.amount).await?;

    let item_id =
        crate::inventory::repository::create_item(&mut tx, variant_id, &input.sku, true, true)
            .await?;

    let location_id = crate::inventory::repository::find_main_location(&mut tx).await?;

    let level_id =
        crate::inventory::repository::create_level(&mut tx, item_id, location_id, 0, 0).await?;

    tx.commit().await?;

    Ok((variant_id, price_id, item_id, level_id))
}
