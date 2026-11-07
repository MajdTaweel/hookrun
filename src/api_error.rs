use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: &'static str,
}
