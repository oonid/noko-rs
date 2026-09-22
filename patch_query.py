import re
with open("tests/customer_provisioning.rs", "r") as f:
    text = f.read()

new_text = text.replace("""sqlx::query(
        r#"
        INSERT INTO actors (id, kind, auth_subject, display_name, active)
        VALUES ($1, 'service', 'service-test', 'Service Test Actor', true)
        "#,
        service_actor_id,
    )""", """sqlx::query(
        r#"
        INSERT INTO actors (id, kind, auth_subject, display_name, active)
        VALUES ($1, 'service', 'service-test', 'Service Test Actor', true)
        "#
    ).bind(service_actor_id)""")

with open("tests/customer_provisioning.rs", "w") as f:
    f.write(new_text)
