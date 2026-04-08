use crate::{
    AppState, Opts,
    http::{
        api::api_router,
        pages::{page_not_found, pages_router},
    },
};
use anyhow::{Context, Result};
use axum::{Router, handler::Handler};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::{ServiceBuilderExt, services::ServeDir};
#[cfg(feature = "dev")]
use tower_livereload::LiveReloadLayer;
use tracing::{debug, info};

mod api;
mod pages;
mod templates;

async fn await_shutdown_signal(state: AppState) {
    state.shutdown_token.cancelled().await;
    debug!("Received shutdown event")
}

pub async fn main(state: AppState, opts: Arc<Opts>) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(opts.http_addr)
        .await
        .with_context(|| format!("Failed to bind to HTTP_ADDR '{}'", opts.http_addr))?;
    info!("HTTP server listening on http://{}", opts.http_addr);

    let middleware = ServiceBuilder::new()
        .compression()
        .trace_for_http()
        .layer(CookieManagerLayer::new());
    #[cfg(feature = "dev")]
    let reload_middleware = LiveReloadLayer::new();
    #[cfg(feature = "dev")]
    let middleware = middleware.layer(reload_middleware);

    axum::serve(
        listener,
        Router::new()
            .merge(pages_router(&state))
            .nest("/api", api_router(&state))
            .fallback_service(
                ServeDir::new(opts.http_site_root.clone())
                    .fallback(page_not_found.with_state(state.clone())),
            )
            .layer(middleware)
            .with_state(state.clone()),
    )
    .with_graceful_shutdown(await_shutdown_signal(state))
    .await
    .context("HTTP server failed to run")
}
