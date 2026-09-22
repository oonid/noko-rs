use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Customer {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub email: String,
    pub phone: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomerAddress {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub label: String,
    pub recipient_name: String,
    pub phone: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub postal_code: String,
    pub country_code: String,
    pub is_default: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomerAddress {
    pub label: String,
    pub recipient_name: String,
    pub phone: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub postal_code: String,
    pub country_code: String,
    pub is_default: bool,
}
