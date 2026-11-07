use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "hookrun",
    })
}
