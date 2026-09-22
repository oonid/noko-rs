use super::model::Actor;
use crate::error::AppError;
use sqlx::PgPool;

pub async fn resolve_by_auth_subject(
    pool: &PgPool,
    auth_subject: &str,
) -> Result<Option<Actor>, AppError> {
    let actor = sqlx::query_as::<_, Actor>(
        r#"
        SELECT id, kind, auth_subject, display_name, active, created_at
        FROM actors
        WHERE auth_subject = $1
        "#,
    )
    .bind(auth_subject)
    .fetch_optional(pool)
    .await?;

    Ok(actor)
}
