use crate::cart::model::{Cart, CartAddress, CartItem};
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn create_or_get_active_cart(
    conn: &mut PgConnection,
    customer_id: Uuid,
) -> Result<Cart, sqlx::Error> {
    // Bounded retry for lifecycle race: if the active Cart that caused
    // the INSERT conflict is completed between INSERT and SELECT,
    // we retry to create/find the next active Cart.
    for _ in 0..3 {
        let _ = sqlx::query(
            r#"
            INSERT INTO carts (customer_id, currency_code, status)
            VALUES ($1, 'IDR', 'active')
            ON CONFLICT (customer_id) WHERE status = 'active'
            DO NOTHING
            "#,
        )
        .bind(customer_id)
        .execute(&mut *conn)
        .await?;

        let cart = sqlx::query_as::<_, Cart>(
            r#"
            SELECT id, customer_id, currency_code, status, created_at, updated_at, completed_at
            FROM carts
            WHERE customer_id = $1 AND status = 'active'
            "#,
        )
        .bind(customer_id)
        .fetch_optional(&mut *conn)
        .await?;

        if let Some(cart) = cart {
            return Ok(cart);
        }
    }

    // Exhausted retries — this is a transient lifecycle race
    Err(sqlx::Error::RowNotFound)
}

pub async fn lock_cart(
    conn: &mut PgConnection,
    cart_id: Uuid,
    customer_id: Uuid,
) -> Result<Option<Cart>, sqlx::Error> {
    sqlx::query_as::<_, Cart>(
        r#"
        SELECT id, customer_id, currency_code, status, created_at, updated_at, completed_at
        FROM carts
        WHERE id = $1 AND customer_id = $2
        FOR UPDATE
        "#,
    )
    .bind(cart_id)
    .bind(customer_id)
    .fetch_optional(conn)
    .await
}

pub async fn add_item_to_cart(
    conn: &mut PgConnection,
    cart_id: Uuid,
    variant_id: Uuid,
    quantity: i64,
) -> Result<Option<CartItem>, sqlx::Error> {
    // 1. Snapshot variant details.
    // Join pricing to get IDR price.
    use sqlx::Row;
    let snapshot = sqlx::query(
        r#"
        SELECT v.title, v.sku, vp.amount
        FROM product_variants v
        JOIN products p ON p.id = v.product_id
        JOIN variant_prices vp ON vp.variant_id = v.id AND vp.currency_code = 'IDR'
        WHERE v.id = $1 AND v.active = true AND p.status = 'active'
        "#,
    )
    .bind(variant_id)
    .fetch_optional(&mut *conn)
    .await?;

    let snapshot = match snapshot {
        Some(s) => s,
        None => return Ok(None), // Variant not found, not active, or no IDR pricing
    };

    let title: String = snapshot.try_get("title")?;
    let sku: String = snapshot.try_get("sku")?;
    let price: i64 = snapshot.try_get("amount")?;

    let item = sqlx::query_as::<_, CartItem>(
        r#"
        INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (cart_id, variant_id) DO UPDATE
        SET quantity = cart_items.quantity + EXCLUDED.quantity,
            updated_at = now()
        RETURNING id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        "#,
    )
    .bind(cart_id)
    .bind(variant_id)
    .bind(title)
    .bind(sku)
    .bind(quantity)
    .bind(price)
    .fetch_one(conn)
    .await?;

    Ok(Some(item))
}

pub async fn update_item_quantity(
    conn: &mut PgConnection,
    cart_id: Uuid,
    item_id: Uuid,
    quantity: i64,
) -> Result<Option<CartItem>, sqlx::Error> {
    sqlx::query_as::<_, CartItem>(
        r#"
        UPDATE cart_items
        SET quantity = $3, updated_at = now()
        WHERE cart_id = $1 AND id = $2
        RETURNING id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        "#,
    )
    .bind(cart_id)
    .bind(item_id)
    .bind(quantity)
    .fetch_optional(conn)
    .await
}

pub async fn remove_item(
    conn: &mut PgConnection,
    cart_id: Uuid,
    item_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        r#"
        DELETE FROM cart_items
        WHERE cart_id = $1 AND id = $2
        "#,
    )
    .bind(cart_id)
    .bind(item_id)
    .execute(conn)
    .await?;

    Ok(res.rows_affected() > 0)
}

pub async fn set_cart_shipping_address(
    conn: &mut PgConnection,
    cart_id: Uuid,
    addr: &crate::customer::model::CustomerAddress,
) -> Result<CartAddress, sqlx::Error> {
    let cart_addr = sqlx::query_as::<_, CartAddress>(
        r#"
        INSERT INTO cart_addresses (
            cart_id, kind, recipient_name, phone, address_line_1, address_line_2,
            city, province, postal_code, country_code
        )
        VALUES ($1, 'shipping', $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (cart_id, kind) DO UPDATE
        SET recipient_name = EXCLUDED.recipient_name,
            phone = EXCLUDED.phone,
            address_line_1 = EXCLUDED.address_line_1,
            address_line_2 = EXCLUDED.address_line_2,
            city = EXCLUDED.city,
            province = EXCLUDED.province,
            postal_code = EXCLUDED.postal_code,
            country_code = EXCLUDED.country_code
        RETURNING id, cart_id, kind, recipient_name, phone, address_line_1, address_line_2, city, province, postal_code, country_code
        "#,
    )
    .bind(cart_id)
    .bind(&addr.recipient_name)
    .bind(&addr.phone)
    .bind(&addr.address_line_1)
    .bind(&addr.address_line_2)
    .bind(&addr.city)
    .bind(&addr.province)
    .bind(&addr.postal_code)
    .bind(&addr.country_code)
    .fetch_one(conn)
    .await?;

    Ok(cart_addr)
}

pub async fn get_cart_items(
    conn: &mut PgConnection,
    cart_id: Uuid,
) -> Result<Vec<CartItem>, sqlx::Error> {
    sqlx::query_as::<_, CartItem>(
        r#"
        SELECT id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        FROM cart_items
        WHERE cart_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(cart_id)
    .fetch_all(conn)
    .await
}

pub async fn get_cart_address(
    conn: &mut PgConnection,
    cart_id: Uuid,
    kind: &str,
) -> Result<Option<CartAddress>, sqlx::Error> {
    sqlx::query_as::<_, CartAddress>(
        r#"
        SELECT id, cart_id, kind, recipient_name, phone, address_line_1, address_line_2, city, province, postal_code, country_code
        FROM cart_addresses
        WHERE cart_id = $1 AND kind = $2
        "#,
    )
    .bind(cart_id)
    .bind(kind)
    .fetch_optional(conn)
    .await
}
