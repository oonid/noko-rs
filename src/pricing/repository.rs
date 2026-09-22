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

pub async fn create_idr_price(
    conn: &mut PgConnection,
    variant_id: Uuid,
    amount: i64,
) -> Result<Uuid, AppError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO variant_prices (variant_id, currency_code, amount)
        VALUES ($1, 'IDR', $2)
        RETURNING id
        "#,
    )
    .bind(variant_id)
    .bind(amount)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}
