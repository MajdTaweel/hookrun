use crate::AppState;
use crate::api_error::{ErrorResponse, database_error_response};
use axum::http::HeaderMap;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use hookrun::{db, delivery::Delivery};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct CreateDeliveryRequest {
    event: String,
}

pub(super) async fn list_deliveries(
    State(state): State<AppState>,
) -> Result<Json<Vec<Delivery>>, (StatusCode, Json<ErrorResponse>)> {
    db::list_deliveries(&state.pool)
        .await
        .map(Json)
        .map_err(database_error_response)
}

pub(super) async fn create_delivery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDeliveryRequest>,
) -> Result<(StatusCode, Json<Delivery>), (StatusCode, Json<ErrorResponse>)> {
    match headers.get("idempotency-key") {
        Some(header_value) => {
            let key = header_value.to_str().map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "invalid Idempotency-Key header",
                    }),
                )
            })?;

            db::create_delivery_with_key(&state.pool, &request.event, key)
                .await
                .map(|delivery| (StatusCode::CREATED, Json(delivery)))
                .map_err(database_error_response)
        }
        None => db::create_delivery(&state.pool, &request.event)
            .await
            .map(|delivery| (StatusCode::CREATED, Json(delivery)))
            .map_err(database_error_response),
    }
}

pub(super) async fn get_delivery(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<Delivery>, (StatusCode, Json<ErrorResponse>)> {
    db::get_delivery(&state.pool, id)
        .await
        .map_err(database_error_response)?
        .map_or(
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "delivery not found",
                }),
            )),
            |delivery| Ok(Json(delivery)),
        )
}
