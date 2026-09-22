cat << 'INNER_EOF' >> tests/db_migrations.rs

#[sqlx::test(migrations = "./migrations")]
async fn test_db_constraints(pool: PgPool) {
    use uuid::Uuid;

    // 1. Actor subject uniqueness (SQLSTATE 23505)
    let actor_id_1 = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_duplicate', 'A')")
        .bind(actor_id_1).execute(&pool).await.unwrap();
    let actor_id_2 = Uuid::new_v4();
    let err = sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_duplicate', 'B')")
        .bind(actor_id_2).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("23505"));

    // 2. Actor kind CHECK constraint (SQLSTATE 23514)
    let actor_id_3 = Uuid::new_v4();
    let err = sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'robot', 'subject_robot', 'C')")
        .bind(actor_id_3).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("23514"));

    // 3. Customer actor_id uniqueness (SQLSTATE 23505)
    let customer_id_1 = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'email1@example.com', 'A', 'B')")
        .bind(customer_id_1).bind(actor_id_1).execute(&pool).await.unwrap();
    let customer_id_2 = Uuid::new_v4();
    let actor_id_new = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, display_name) VALUES ($1, 'human', 'subject_other', 'D')")
        .bind(actor_id_new).execute(&pool).await.unwrap();
    let err = sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'email2@example.com', 'C', 'D')")
        .bind(customer_id_2).bind(actor_id_1).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("23505"));

    // 4. Customer email uniqueness (SQLSTATE 23505)
    let customer_id_3 = Uuid::new_v4();
    let err = sqlx::query("INSERT INTO customers (id, actor_id, email, first_name, last_name) VALUES ($1, $2, 'email1@example.com', 'E', 'F')")
        .bind(customer_id_3).bind(actor_id_new).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("23505"));

    // 5. CustomerAddress customer_id FK (SQLSTATE 23503)
    let missing_customer_id = Uuid::new_v4();
    let err = sqlx::query("INSERT INTO customer_addresses (customer_id, label, recipient_name, address_line_1, city, province, postal_code, country_code) VALUES ($1, 'Home', 'A', '123 St', 'City', 'Prov', '12345', 'US')")
        .bind(missing_customer_id).execute(&pool).await.unwrap_err();
    assert!(err.to_string().contains("23503"));
}
INNER_EOF
