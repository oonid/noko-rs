use crate::AppState;
use axum::Router;

pub mod store;

pub fn router() -> Router<AppState> {
    Router::new().nest("/store", store::router())
}
