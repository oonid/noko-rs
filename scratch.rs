use axum::{extract::FromRequestParts, http::request::Parts, async_trait};
use crate::app::AppState;
use crate::error::AppError;
use crate::auth::AuthContext;
use crate::actor::repository::get_actor_by_id;

pub struct OpsCaller {
    pub actor: AuthContext,
}
