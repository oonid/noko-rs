use crate::app::AppState;
use axum::{Router, routing::post};

pub mod catalog;
pub mod customers;
pub mod inventory;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/catalog/variants", post(catalog::handle_create_variant))
        .route(
            "/inventory/adjustments",
            post(inventory::handle_adjust_inventory),
        )
        .route("/customers", post(customers::handle_provision_customer))
}
