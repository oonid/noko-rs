use crate::cart::model::{Cart, CartAddress, CartItem};
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn create_or_get_active_cart(
    conn: &mut PgConnection,
    customer_id: Uuid,
) -> Result<Cart, sqlx::Error> {
    // Attempt to insert, DO NOTHING if conflict on active cart index
    let _ = sqlx::query!(
        r#"
        INSERT INTO carts (customer_id, currency_code, status)
        VALUES ($1, 'IDR', 'active')
        ON CONFLICT (customer_id) WHERE status = 'active'
        DO NOTHING
        "#,
        customer_id
    )
    .execute(&mut *conn)
    .await?;

    // Now select the active cart
    let cart = sqlx::query_as!(
        Cart,
        r#"
        SELECT id, customer_id, currency_code, status, created_at, updated_at, completed_at
        FROM carts
        WHERE customer_id = $1 AND status = 'active'
        "#,
        customer_id
    )
    .fetch_one(conn)
    .await?;

    Ok(cart)
}

pub async fn lock_active_cart(
    conn: &mut PgConnection,
    cart_id: Uuid,
    customer_id: Uuid,
) -> Result<Option<Cart>, sqlx::Error> {
    sqlx::query_as!(
        Cart,
        r#"
        SELECT id, customer_id, currency_code, status, created_at, updated_at, completed_at
        FROM carts
        WHERE id = $1 AND customer_id = $2 AND status = 'active'
        FOR UPDATE
        "#,
        cart_id,
        customer_id
    )
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
    let snapshot = sqlx::query!(
        r#"
        SELECT v.title, v.sku, p.amount
        FROM product_variants v
        JOIN variant_prices p ON p.variant_id = v.id AND p.currency_code = 'IDR'
        WHERE v.id = $1 AND v.active = true
        "#,
        variant_id
    )
    .fetch_optional(&mut *conn)
    .await?;

    let snapshot = match snapshot {
        Some(s) => s,
        None => return Ok(None), // Variant not found, not active, or no IDR pricing
    };

    let price = snapshot.amount; // Assuming price is not null in query result actually wait, variant_prices price might be optional? Let's check schema. We will use query_as or just struct.

    let item = sqlx::query_as!(
        CartItem,
        r#"
        INSERT INTO cart_items (cart_id, variant_id, variant_title, sku, quantity, unit_price)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (cart_id, variant_id) DO UPDATE
        SET quantity = cart_items.quantity + EXCLUDED.quantity,
            updated_at = now()
        RETURNING id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        "#,
        cart_id,
        variant_id,
        snapshot.title,
        snapshot.sku,
        quantity,
        price
    )
    .fetch_one(conn)
    .await?;

    Ok(Some(item))
}

pub async fn update_item_quantity(
    conn: &mut PgConnection,
    cart_id: Uuid,
    variant_id: Uuid,
    quantity: i64,
) -> Result<Option<CartItem>, sqlx::Error> {
    sqlx::query_as!(
        CartItem,
        r#"
        UPDATE cart_items
        SET quantity = $3, updated_at = now()
        WHERE cart_id = $1 AND id = $2
        RETURNING id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        "#,
        cart_id,
        variant_id,
        quantity
    )
    .fetch_optional(conn)
    .await
}

pub async fn remove_item(
    conn: &mut PgConnection,
    cart_id: Uuid,
    variant_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        r#"
        DELETE FROM cart_items
        WHERE cart_id = $1 AND id = $2
        "#,
        cart_id,
        variant_id
    )
    .execute(conn)
    .await?;

    Ok(res.rows_affected() > 0)
}

pub async fn set_cart_shipping_address(
    conn: &mut PgConnection,
    cart_id: Uuid,
    addr: &crate::customer::model::CustomerAddress,
) -> Result<CartAddress, sqlx::Error> {
    let cart_addr = sqlx::query_as!(
        CartAddress,
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
        cart_id,
        addr.recipient_name,
        addr.phone,
        addr.address_line_1,
        addr.address_line_2,
        addr.city,
        addr.province,
        addr.postal_code,
        addr.country_code
    )
    .fetch_one(conn)
    .await?;

    Ok(cart_addr)
}

pub async fn get_cart_items(
    conn: &mut PgConnection,
    cart_id: Uuid,
) -> Result<Vec<CartItem>, sqlx::Error> {
    sqlx::query_as!(
        CartItem,
        r#"
        SELECT id, cart_id, variant_id, variant_title, sku, quantity, unit_price, created_at, updated_at
        FROM cart_items
        WHERE cart_id = $1
        ORDER BY created_at ASC
        "#,
        cart_id
    )
    .fetch_all(conn)
    .await
}

pub async fn get_cart_address(
    conn: &mut PgConnection,
    cart_id: Uuid,
    kind: &str,
) -> Result<Option<CartAddress>, sqlx::Error> {
    sqlx::query_as!(
        CartAddress,
        r#"
        SELECT id, cart_id, kind, recipient_name, phone, address_line_1, address_line_2, city, province, postal_code, country_code
        FROM cart_addresses
        WHERE cart_id = $1 AND kind = $2
        "#,
        cart_id,
        kind
    )
    .fetch_optional(conn)
    .await
}
