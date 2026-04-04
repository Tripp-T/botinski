use crate::*;
use axum::{
    Router, debug_handler, extract::State, http::StatusCode, response::IntoResponse, routing::get,
};

async fn shutdown_signal(signal: oneshot::Receiver<()>) {
    if let Err(e) = signal.await {
        error!("Failed to await shutdown signal: {e}")
    };
}

pub async fn main(state: AppState) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(state.opts.http_addr)
        .await
        .with_context(|| format!("Failed to bind to HTTP_ADDR '{}'", state.opts.http_addr))?;
    info!("HTTP server listening on http://{}", state.opts.http_addr);
    axum::serve(
        listener,
        Router::new()
            .route("/", get(response_not_found))
            .with_state(state.clone()),
    )
    .with_graceful_shutdown(shutdown_signal(state.register_shutdown_callback().await))
    .await
    .context("HTTP server failed to run")
}

#[debug_handler]
async fn response_not_found(_: State<AppState>) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}
