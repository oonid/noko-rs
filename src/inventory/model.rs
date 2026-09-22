use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InventoryLocation {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InventoryItem {
    pub id: Uuid,
    pub variant_id: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InventoryLevel {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub stocked_quantity: i64,
    pub reserved_quantity: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InventoryAdjustment {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub delta: i64,
    pub reason: String,
    pub note: Option<String>,
    pub actor_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryAvailability {
    pub inventory_item_id: Uuid,
    pub inventory_level_id: Uuid,
    pub stocked_quantity: i64,
    pub reserved_quantity: i64,
    pub available_quantity: i64,
}
