use crate::actor::{ActorKind, resolve_by_auth_subject};
use crate::app::AppState;
use crate::customer::repository::get_customer_by_actor_id;
use crate::error::AppError;
use axum::{extract::FromRequestParts, http::request::Parts};
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
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.config.auth_mode != "dev_header" {
            return Err(AppError::internal("Invalid auth mode"));
        }

        let auth_subject = parts
            .headers
            .get("x-dev-auth-subject")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("AUTH_REQUIRED"))?;

        let actor = resolve_by_auth_subject(&state.pool, auth_subject)
            .await?
            .ok_or_else(|| AppError::unauthorized("AUTH_SUBJECT_UNKNOWN"))?;

        if !actor.active {
            return Err(AppError::unauthorized("ACTOR_INACTIVE"));
        }

        Ok(AuthContext {
            actor_id: actor.id,
            actor_kind: actor.kind,
            auth_subject: actor.auth_subject,
        })
    }
}

impl FromRequestParts<AppState> for AuthenticatedCustomer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthContext::from_request_parts(parts, state).await?;

        let customer = get_customer_by_actor_id(&state.pool, auth.actor_id)
            .await?
            .ok_or_else(|| AppError::forbidden("CUSTOMER_REQUIRED"))?;

        Ok(AuthenticatedCustomer(CustomerContext {
            auth,
            customer_id: customer.id,
        }))
    }
}

pub struct OpsCaller {
    pub actor: AuthContext,
}

impl FromRequestParts<AppState> for OpsCaller {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Require Token matching config
        let expected_token = state
            .config
            .nocodb_service_token
            .as_deref()
            .ok_or_else(|| AppError::unauthorized("NO_SERVICE_TOKEN_CONFIGURED"))?;

        let expected_actor_id = state
            .config
            .nocodb_service_actor_id
            .ok_or_else(|| AppError::unauthorized("NO_SERVICE_ACTOR_CONFIGURED"))?;

        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("AUTH_REQUIRED"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("INVALID_TOKEN_FORMAT"))?;

        if token != expected_token {
            return Err(AppError::unauthorized("INVALID_TOKEN"));
        }

        let actor = crate::actor::repository::get_actor_by_id(&state.pool, expected_actor_id)
            .await?
            .ok_or_else(|| AppError::unauthorized("SERVICE_ACTOR_NOT_FOUND"))?;

        if !actor.active {
            return Err(AppError::unauthorized("ACTOR_INACTIVE"));
        }
        if actor.kind != ActorKind::Service {
            return Err(AppError::unauthorized("ACTOR_NOT_SERVICE"));
        }

        Ok(OpsCaller {
            actor: AuthContext {
                actor_id: actor.id,
                actor_kind: actor.kind,
                auth_subject: actor.auth_subject,
            },
        })
    }
}
