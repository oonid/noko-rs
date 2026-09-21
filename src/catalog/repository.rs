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
        SELECT v.id, v.product_id, v.sku, v.title, v.active, v.created_at, v.updated_at
        FROM product_variants v
        JOIN products p ON v.product_id = p.id
        WHERE v.id = $1 AND v.active = true AND p.status = 'active'
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("VARIANT_NOT_FOUND"))?;

    Ok(variant)
}
