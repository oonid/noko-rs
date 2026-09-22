import re

with open("src/inventory/repository.rs", "r") as f:
    content = f.read()

new_insert = r"""pub async fn create_item(
    conn: &mut PgConnection,
    variant_id: Uuid,
    sku: &str,
    requires_shipping: bool,
    tracked: bool,
) -> Result<Uuid, AppError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO inventory_items (variant_id)
        VALUES ($1)
        RETURNING id
        "#
    )
    .bind(variant_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}"""

content = re.sub(r'pub async fn create_item.*?Ok\(id\)\n}', new_insert, content, flags=re.DOTALL)

with open("src/inventory/repository.rs", "w") as f:
    f.write(content)

