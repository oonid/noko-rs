use crate::AppState;
use axum::Router;

pub mod me;
pub mod products;
pub mod carts;

pub fn router() -> Router<AppState> {
    Router::new().merge(products::router()).merge(me::router()).merge(carts::router())
}
