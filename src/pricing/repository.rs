use crate::error::AppError;
use crate::pricing::model::VariantPrice;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn get_idr_price(
    conn: &mut PgConnection,
    variant_id: Uuid,
) -> Result<VariantPrice, AppError> {
    let price = sqlx::query_as::<_, VariantPrice>(
        r#"
        SELECT id, variant_id, currency_code, amount, created_at, updated_at
        FROM variant_prices
        WHERE variant_id = $1 AND currency_code = 'IDR'
        "#,
    )
    .bind(variant_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("PRICE_NOT_FOUND"))?;

    Ok(price)
}
