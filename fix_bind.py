import re

with open("tests/operations.rs", "r") as f:
    content = f.read()

# I will just write a simpler regex or replace manually
content = content.replace('sqlx::query(\n        "INSERT INTO actors (id, kind, auth_subject, active) VALUES ($1, \'service\', $2, true)",\n        actor_id,\n        format!("sub_{}", actor_id)\n    )', 'sqlx::query("INSERT INTO actors (id, kind, auth_subject, active) VALUES ($1, \'service\', $2, true)").bind(actor_id).bind(format!("sub_{}", actor_id))')

content = content.replace('sqlx::query("INSERT INTO locations (id, name, active) VALUES ($1, $2, true)", location_id, "Main")', 'sqlx::query("INSERT INTO locations (id, name, active) VALUES ($1, $2, true)").bind(location_id).bind("Main")')

content = content.replace('sqlx::query(\n        "INSERT INTO products (id, sku, title, is_active) VALUES ($1, $2, \'test product\', true)",\n        product_id,\n        format!("PROD_{}", product_id)\n    )', 'sqlx::query("INSERT INTO products (id, sku, title, is_active) VALUES ($1, $2, \'test product\', true)").bind(product_id).bind(format!("PROD_{}", product_id))')

with open("tests/operations.rs", "w") as f:
    f.write(content)
