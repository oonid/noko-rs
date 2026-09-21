use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn test_migrations_create_main_location(pool: PgPool) {
    // The pool is already migrated by sqlx::test, so MAIN should exist
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_repeated_migration(pool: PgPool) {
    // Run migrations again
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Verify it's still exactly 1 MAIN location
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_migration_failure_prevents_startup() {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noko-rs"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env(
            "DATABASE_URL",
            "postgres://nonexistent:nonexistent@127.0.0.1:5432/nonexistent",
        )
        .env("BIND_ADDR", "127.0.0.1:39124")
        .spawn()
        .expect("start binary");

    let exit_status = child.wait().expect("child exit");
    assert!(!exit_status.success());
}
