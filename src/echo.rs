use crate::api_error::ErrorResponse;
use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(super) struct EchoRequest {
    event: String,
}

#[derive(Serialize)]
pub(super) struct EchoResponse {
    event: String,
}

pub(super) async fn echo(
    Json(echo_request): Json<EchoRequest>,
) -> Result<Json<EchoResponse>, (StatusCode, Json<ErrorResponse>)> {
    if echo_request.event.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "event must not be blank",
            }),
        ));
    }

    Ok(Json(EchoResponse {
        event: echo_request.event,
    }))
}
