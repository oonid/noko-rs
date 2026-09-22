use axum::{
    extract::{FromRequest, FromRequestParts, rejection::{JsonRejection, PathRejection}},
    http::request::Parts,
    Json as AxumJson,
    extract::Path as AxumPath,
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
            Err(rejection) => {
                let message = rejection.body_text();
                Err(AppError::bad_request("INVALID_JSON", message))
            }
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
            Err(rejection) => {
                let message = rejection.body_text();
                Err(AppError::bad_request("INVALID_PATH", message))
            }
        }
    }
}
