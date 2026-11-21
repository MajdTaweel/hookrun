use axum::{Json, http::StatusCode};
use hookrun::db::DbError;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: &'static str,
}

pub(crate) fn database_error_response(error: DbError) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        DbError::BlankEvent => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "event must not be blank",
            }),
        ),
        DbError::InvalidId(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "delivery ID exceeds the supported range",
            }),
        ),
        DbError::BlankIdempotencyKey => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "idempotency key must not be blank",
            }),
        ),
        DbError::IdempotencyConflict => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "idempotency key was already used with a different event",
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "internal server error",
            }),
        ),
    }
}
