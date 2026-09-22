use axum::{
    Json as AxumJson,
    extract::Path as AxumPath,
    extract::{FromRequest, FromRequestParts, rejection::JsonRejection},
    http::request::Parts,
};
use serde::de::DeserializeOwned;

use crate::error::AppError;

pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    AxumJson<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match AxumJson::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(_rejection) => Err(AppError::bad_request(
                "INVALID_JSON",
                "Invalid JSON request body",
            )),
        }
    }
}

pub struct Path<T>(pub T);

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AxumPath::<T>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(_rejection) => Err(AppError::bad_request(
                "INVALID_PATH",
                "Invalid path parameter",
            )),
        }
    }
}

use axum::response::{IntoResponse, Response};

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}
