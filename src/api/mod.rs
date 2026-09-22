use crate::AppState;
use axum::Router;

pub mod ops;
pub mod store;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/store", store::router())
        .nest("/ops", ops::router())
}
