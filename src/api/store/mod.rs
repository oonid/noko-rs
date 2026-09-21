use crate::AppState;
use axum::Router;

pub mod products;

pub fn router() -> Router<AppState> {
    Router::new().merge(products::router())
}
