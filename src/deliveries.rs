use crate::DeliveryStore;
use crate::api_error::ErrorResponse;
use crate::delivery::{Delivery, DeliveryStatus};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub(super) struct CreateDeliveryRequest {
    event: String,
}

pub(super) async fn list_deliveries(
    State(delivery_store): State<Arc<Mutex<DeliveryStore>>>,
) -> Json<Vec<Delivery>> {
    let snapshot = {
        let guard = delivery_store.lock().expect("delivery lock poisoned");
        guard.deliveries.to_vec()
    };

    Json(snapshot)
}

pub(super) async fn create_delivery(
    State(delivery_store): State<Arc<Mutex<DeliveryStore>>>,
    Json(request): Json<CreateDeliveryRequest>,
) -> Result<(StatusCode, Json<Delivery>), (StatusCode, Json<ErrorResponse>)> {
    if request.event.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "event must not be blank",
            }),
        ));
    }

    let new_delivery = {
        let mut guard = delivery_store.lock().expect("delivery lock poisoned");

        let new_delivery = Delivery {
            id: guard.next_id,
            event: request.event,
            attempts: 0,
            status: DeliveryStatus::Pending,
        };

        guard.next_id += 1;
        guard.deliveries.push(new_delivery.clone());

        new_delivery
    };

    Ok((StatusCode::CREATED, Json(new_delivery)))
}

pub(super) async fn get_delivery(
    State(delivery_store): State<Arc<Mutex<DeliveryStore>>>,
    Path(id): Path<u64>,
) -> Result<Json<Delivery>, (StatusCode, Json<ErrorResponse>)> {
    let delivery = {
        let guard = delivery_store.lock().expect("delivery lock poisoned");
        guard
            .deliveries
            .iter()
            .find(|delivery| delivery.id == id)
            .cloned()
    };

    delivery.map_or(
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "delivery not found",
            }),
        )),
        |delivery| Ok(Json(delivery)),
    )
}
