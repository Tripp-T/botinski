use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};

use crate::AppState;

pub fn api_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/healthcheck", get(healthcheck))
}

async fn healthcheck(state: State<AppState>) -> impl IntoResponse {
    if state.db.is_closed() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "DB closed");
    }
    (StatusCode::OK, "OK")
}
