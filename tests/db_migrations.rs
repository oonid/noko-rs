use noko_rs::db;
use sqlx::PgPool;

#[sqlx::test(migrations = false)]
async fn test_run_migrations_fresh_db(pool: PgPool) {
    db::run_migrations(&pool)
        .await
        .expect("initial migration should succeed");
    db::run_migrations(&pool)
        .await
        .expect("repeated migration should be a no-op and succeed");
}

#[sqlx::test(migrations = false)]
async fn test_run_migrations_conflict(pool: PgPool) {
    sqlx::query("CREATE TABLE products (id int PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let err = db::run_migrations(&pool).await;
    assert!(err.is_err(), "migration should fail due to conflict");
}
