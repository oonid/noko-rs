use super::model::{CreateCustomerAddress, Customer, CustomerAddress};
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_customer_by_id(
    pool: &PgPool,
    customer_id: Uuid,
) -> Result<Option<Customer>, AppError> {
    let customer = sqlx::query_as::<_, Customer>(
        r#"
        SELECT id, actor_id, email, phone, first_name, last_name, created_at, updated_at
        FROM customers
        WHERE id = $1
        "#,
    )
    .bind(customer_id)
    .fetch_optional(pool)
    .await?;

    Ok(customer)
}

pub async fn get_customer_by_actor_id(
    pool: &PgPool,
    actor_id: Uuid,
) -> Result<Option<Customer>, AppError> {
    let customer = sqlx::query_as::<_, Customer>(
        r#"
        SELECT id, actor_id, email, phone, first_name, last_name, created_at, updated_at
        FROM customers
        WHERE actor_id = $1
        "#,
    )
    .bind(actor_id)
    .fetch_optional(pool)
    .await?;

    Ok(customer)
}

pub async fn get_addresses(
    pool: &PgPool,
    customer_id: Uuid,
) -> Result<Vec<CustomerAddress>, AppError> {
    let addresses = sqlx::query_as::<_, CustomerAddress>(
        r#"
        SELECT id, customer_id, label, recipient_name, phone, address_line_1, address_line_2,
               city, province, postal_code, country_code, is_default, created_at, updated_at
        FROM customer_addresses
        WHERE customer_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(customer_id)
    .fetch_all(pool)
    .await?;

    Ok(addresses)
}

pub async fn create_address(
    pool: &PgPool,
    customer_id: Uuid,
    address: CreateCustomerAddress,
) -> Result<CustomerAddress, AppError> {
    let new_address = sqlx::query_as::<_, CustomerAddress>(
        r#"
        INSERT INTO customer_addresses (
            customer_id, label, recipient_name, phone, address_line_1, address_line_2,
            city, province, postal_code, country_code, is_default
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id, customer_id, label, recipient_name, phone, address_line_1, address_line_2,
                  city, province, postal_code, country_code, is_default, created_at, updated_at
        "#,
    )
    .bind(customer_id)
    .bind(address.label)
    .bind(address.recipient_name)
    .bind(address.phone)
    .bind(address.address_line_1)
    .bind(address.address_line_2)
    .bind(address.city)
    .bind(address.province)
    .bind(address.postal_code)
    .bind(address.country_code)
    .bind(address.is_default)
    .fetch_one(pool)
    .await?;

    Ok(new_address)
}
