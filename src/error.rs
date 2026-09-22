use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation error: {message}")]
    Validation {
        code: String,
        message: String,
        details: Option<Value>,
    },
    #[error("not found: {message}")]
    NotFound { code: String, message: String },
    #[error("forbidden: {message}")]
    Forbidden { code: String, message: String },
    #[error("unauthorized: {message}")]
    Unauthorized { code: String, message: String },
    #[error("conflict: {message}")]
    Conflict {
        code: String,
        message: String,
        details: Option<Value>,
    },
    #[error("bad request: {message}")]
    BadRequest { code: String, message: String },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn validation(code: impl Into<String>) -> Self {
        let code = code.into();
        let message = format!("Validation failed: {}", code);
        Self::Validation {
            message,
            code,
            details: None,
        }
    }

    pub fn validation_msg(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn validation_with_details(code: impl Into<String>, details: Value) -> Self {
        let code = code.into();
        let message = format!("Validation failed: {}", code);
        Self::Validation {
            message,
            code,
            details: Some(details),
        }
    }

    pub fn not_found(code: impl Into<String>) -> Self {
        let code = code.into();
        Self::NotFound {
            message: code.clone(),
            code,
        }
    }

    pub fn forbidden(code: impl Into<String>) -> Self {
        let code = code.into();
        Self::Forbidden {
            message: code.clone(),
            code,
        }
    }

    pub fn unauthorized(code: impl Into<String>) -> Self {
        let code = code.into();
        Self::Unauthorized {
            message: code.clone(),
            code,
        }
    }

    pub fn conflict(code: impl Into<String>) -> Self {
        let code = code.into();
        Self::Conflict {
            message: code.clone(),
            code,
            details: None,
        }
    }

    pub fn conflict_with_details(code: impl Into<String>, details: Value) -> Self {
        let code = code.into();
        Self::Conflict {
            message: code.clone(),
            code,
            details: Some(details),
        }
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[derive(Serialize)]
struct ErrorResponseBody {
    code: String,
    message: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, retryable, details) = match self {
            AppError::Validation {
                code,
                message,
                details,
            } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                code,
                message,
                false,
                details,
            ),
            AppError::NotFound { code, message } => {
                (StatusCode::NOT_FOUND, code, message, false, None)
            }
            AppError::Forbidden { code, message } => {
                (StatusCode::FORBIDDEN, code, message, false, None)
            }
            AppError::Unauthorized { code, message } => {
                (StatusCode::UNAUTHORIZED, code, message, false, None)
            }
            AppError::Conflict {
                code,
                message,
                details,
            } => (StatusCode::CONFLICT, code, message, false, details),
            AppError::BadRequest { code, message } => {
                (StatusCode::BAD_REQUEST, code, message, false, None)
            }
            AppError::Database(err) => {
                tracing::error!(error = %err, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR".to_string(),
                    "Internal database error".to_string(),
                    false,
                    None,
                )
            }
            AppError::Internal(err) => {
                tracing::error!(error = %err, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR".to_string(),
                    "Internal server error".to_string(),
                    false,
                    None,
                )
            }
        };

        let body = Json(ErrorResponseBody {
            code,
            message,
            retryable,
            details,
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::json;

    #[tokio::test]
    async fn test_error_status_codes_and_json() {
        let res = AppError::validation("INVALID_INPUT").into_response();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["code"], "INVALID_INPUT");
        assert_eq!(json["retryable"], false);

        let res = AppError::validation_msg("INVALID_FIELD", "custom message").into_response();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let res = AppError::validation_with_details("INVALID_DETAIL", json!({ "field": "amount" }))
            .into_response();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let res = AppError::not_found("VARIANT_NOT_FOUND").into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        let res = AppError::forbidden("FORBIDDEN_ACCESS").into_response();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        let res = AppError::unauthorized("MISSING_TOKEN").into_response();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let res = AppError::conflict("CART_PRICE_CHANGED").into_response();
        assert_eq!(res.status(), StatusCode::CONFLICT);

        let res = AppError::conflict_with_details(
            "INSUFFICIENT_INVENTORY",
            json!({ "requested": 5, "available": 3 }),
        )
        .into_response();
        assert_eq!(res.status(), StatusCode::CONFLICT);

        let db_err: AppError = sqlx::Error::RowNotFound.into();
        let res = db_err.into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let res = AppError::internal("something broke").into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
