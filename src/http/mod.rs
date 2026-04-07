use {
    crate::AppState,
    anyhow::{Context, Result},
    axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get},
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
            .nest("/api", api_router(&state))
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

fn api_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/healthcheck", get(healthcheck))
}

async fn healthcheck(state: State<AppState>) -> impl IntoResponse {
    if state.db.is_closed() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "DB closed");
    }
    (StatusCode::OK, "OK")
}
