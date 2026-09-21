use axum::{
    Json, Router,
    extract::State,
    http::{Request, Response, StatusCode, header::HeaderName},
    response::IntoResponse,
    routing::get,
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
}

#[derive(Clone, Copy)]
pub struct HttpMakeSpan;

impl<B> tower_http::trace::MakeSpan<B> for HttpMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> tracing::Span {
        let request_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();

        tracing::info_span!(
            "http_request",
            request_id = %request_id,
            method = %request.method(),
            uri = %request.uri(),
        )
    }
}

#[derive(Clone, Copy)]
pub struct HttpOnResponse;

impl<B> tower_http::trace::OnResponse<B> for HttpOnResponse {
    fn on_response(self, response: &Response<B>, latency: Duration, _span: &tracing::Span) {
        tracing::info!(
            status = response.status().as_u16(),
            latency_ms = latency.as_millis(),
            "http_response"
        );
    }
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn health_ready(State(state): State<AppState>) -> impl IntoResponse {
    match crate::db::check_readiness(&state.pool).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "database": "connected" })),
        ),
        Err(err) => {
            tracing::error!(error = %err, "Readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "unavailable",
                    "error": "database unreachable"
                })),
            )
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    let x_request_id = HeaderName::from_static("x-request-id");

    Router::new()
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .merge(crate::api::router())
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(HttpMakeSpan)
                .on_response(HttpOnResponse),
        )
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid))
}
