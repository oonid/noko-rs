use crate::actor::{ActorKind, resolve_by_auth_subject};
use crate::app::AppState;
use crate::customer::repository::get_customer_by_actor_id;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub actor_id: Uuid,
    pub actor_kind: ActorKind,
    pub auth_subject: String,
}

#[derive(Debug, Clone)]
pub struct CustomerContext {
    pub auth: AuthContext,
    pub customer_id: Uuid,
}

pub struct AuthenticatedCustomer(pub CustomerContext);

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.config.auth_mode != "dev_header" {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid auth mode"));
        }

        let auth_subject = parts
            .headers
            .get("x-dev-auth-subject")
            .and_then(|value| value.to_str().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing x-dev-auth-subject header",
            ))?;

        let actor = resolve_by_auth_subject(&state.pool, auth_subject)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::UNAUTHORIZED, "Unknown auth subject"))?;

        if !actor.active {
            return Err((StatusCode::UNAUTHORIZED, "Inactive actor"));
        }

        Ok(AuthContext {
            actor_id: actor.id,
            actor_kind: actor.kind,
            auth_subject: actor.auth_subject,
        })
    }
}

impl FromRequestParts<AppState> for AuthenticatedCustomer {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthContext::from_request_parts(parts, state).await?;

        let customer = get_customer_by_actor_id(&state.pool, auth.actor_id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::FORBIDDEN, "Not a customer"))?;

        Ok(AuthenticatedCustomer(CustomerContext {
            auth,
            customer_id: customer.id,
        }))
    }
}
