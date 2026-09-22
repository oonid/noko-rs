use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Cart {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub currency_code: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CartItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub variant_id: Uuid,
    pub variant_title: String,
    pub sku: String,
    pub quantity: i64,
    pub unit_price: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CartAddress {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub kind: String,
    pub recipient_name: String,
    pub phone: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub postal_code: String,
    pub country_code: String,
}
