use {
    crate::AppState,
    anyhow::{Context, Result},
    axum::{Router, extract::State, http::StatusCode, response::IntoResponse},
    tower::ServiceBuilder,
    tower_http::ServiceBuilderExt,
    tracing::info,
};

async fn await_shutdown_signal(state: AppState) {
    state.shutdown_token.cancelled().await
}

pub async fn main(state: AppState) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(state.opts.http_addr)
        .await
        .with_context(|| format!("Failed to bind to HTTP_ADDR '{}'", state.opts.http_addr))?;
    info!("HTTP server listening on http://{}", state.opts.http_addr);
    axum::serve(
        listener,
        Router::new()
            .fallback(response_not_found)
            .layer(ServiceBuilder::new().compression().trace_for_http())
            .with_state(state.clone()),
    )
    .with_graceful_shutdown(await_shutdown_signal(state))
    .await
    .context("HTTP server failed to run")
}

async fn response_not_found(_: State<AppState>) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}
