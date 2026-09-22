import os

os.makedirs("src/cart", exist_ok=True)

with open("src/cart/mod.rs", "w") as f:
    f.write("""pub mod model;
pub mod repository;
""")

with open("src/cart/model.rs", "w") as f:
    f.write("""use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cart {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub currency_code: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub variant_id: Uuid,
    pub variant_title: String,
    pub sku: String,
    pub quantity: i64,
    pub unit_price: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
""")

