use crate::AppState;
use axum::Router;

pub mod carts;
pub mod me;
pub mod products;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(products::router())
        .merge(me::router())
        .merge(carts::router())
}
