use crate::catalog::model::ProductVariant;
use crate::error::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn get_active_variant(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<ProductVariant, AppError> {
    let variant = sqlx::query_as::<_, ProductVariant>(
        r#"
        SELECT id, product_id, sku, title, active, created_at, updated_at
        FROM product_variants
        WHERE id = $1 AND active = true
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("VARIANT_NOT_FOUND"))?;

    Ok(variant)
}
