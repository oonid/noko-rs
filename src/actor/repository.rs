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

pub async fn get_actor_by_id(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
) -> Result<Option<Actor>, AppError> {
    let actor = sqlx::query_as::<_, Actor>(
        r#"
        SELECT id, kind, auth_subject, display_name, active, created_at
        FROM actors
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(actor)
}

pub async fn create_human_actor(
    conn: &mut sqlx::PgConnection,
    auth_subject: &str,
    display_name: &str,
) -> Result<Actor, AppError> {
    let actor = sqlx::query_as::<_, Actor>(
        r#"
        INSERT INTO actors (kind, auth_subject, display_name, active)
        VALUES ('human', $1, $2, true)
        RETURNING id, kind, auth_subject, display_name, active, created_at
        "#,
    )
    .bind(auth_subject)
    .bind(display_name)
    .fetch_one(conn)
    .await?;

    Ok(actor)
}
