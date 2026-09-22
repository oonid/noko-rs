use noko_rs::db;
use sqlx::PgPool;

#[sqlx::test(migrations = false)]
async fn test_run_migrations_fresh_db(pool: PgPool) {
    db::run_migrations(&pool)
        .await
        .expect("initial migration should succeed");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "MAIN location should be created after first migration"
    );

    db::run_migrations(&pool)
        .await
        .expect("repeated migration should be a no-op and succeed");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_locations WHERE code = 'MAIN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "MAIN location should still be exactly 1 after repeated migration"
    );
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

struct DbDropGuard {
    base_url: String,
    db_name: String,
}

impl Drop for DbDropGuard {
    fn drop(&mut self) {
        let base_url = self.base_url.clone();
        let db_name = self.db_name.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                rt.block_on(async {
                    use sqlx::Connection;
                    if let Ok(mut conn) = sqlx::PgConnection::connect(&base_url).await {
                        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
                            db_name
                        )))
                        .execute(&mut conn)
                        .await;
                    }
                });
            }
        })
        .join()
        .ok();
    }
}

#[tokio::test]
#[allow(clippy::zombie_processes)]
async fn test_runtime_migration_failure_prevents_listener() {
    use sqlx::{Connection, Executor, PgConnection};
    use std::process::Command;
    use std::time::Duration;
    use uuid::Uuid;

    let base_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut conn = PgConnection::connect(&base_url).await.unwrap();

    let db_name = format!("test_conflict_{}", Uuid::new_v4().simple());

    let _guard = DbDropGuard {
        base_url: base_url.clone(),
        db_name: db_name.clone(),
    };

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{}\"",
        db_name
    )))
    .execute(&mut conn)
    .await
    .unwrap();

    // construct new url
    // Assumes DATABASE_URL format is postgres://user:pass@host:port/dbname
    let (base_without_db, _old_db) = base_url.rsplit_once('/').unwrap();
    let new_url = format!("{}/{}", base_without_db, db_name);

    let mut new_conn = PgConnection::connect(&new_url).await.unwrap();
    new_conn
        .execute("CREATE TABLE products (id integer PRIMARY KEY)")
        .await
        .unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let bind_addr = listener.local_addr().unwrap().to_string();
    drop(listener); // free the port for the binary to use

    // start binary
    let bin_path = env!("CARGO_BIN_EXE_noko-rs");

    let mut child = Command::new(bin_path)
        .env("DATABASE_URL", &new_url)
        .env("BIND_ADDR", &bind_addr)
        .spawn()
        .unwrap();

    // wait for it to exit
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();
    let mut exited = false;
    let mut status = None;

    while start.elapsed() < timeout {
        if let Some(s) = child.try_wait().unwrap() {
            exited = true;
            status = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if !exited {
        child.kill().unwrap();
        child.wait().unwrap(); // fix zombie process clippy warning
        drop(new_conn); // ensure lock is released before drop guard runs on panic
        panic!("Binary did not exit fast enough on migration failure");
    }

    // drop connection so DropGuard can drop the database cleanly (WITH FORCE also helps)
    drop(new_conn);
    drop(conn);

    assert!(
        !status.unwrap().success(),
        "Binary should exit with non-zero status on migration failure"
    );

    let stream = std::net::TcpStream::connect(&bind_addr);
    assert!(
        stream.is_err(),
        "Should not be able to connect to the listener port"
    );
}
