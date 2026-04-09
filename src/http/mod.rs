use crate::{
    AppState, Opts,
    http::{
        api::api_router,
        middleware::{middleware_error_formatting, middleware_http_trace},
        pages::{page_not_found, pages_router},
    },
};
use anyhow::{Context, Result};
use axum::{Extension, Router, handler::Handler, http::StatusCode, response::IntoResponse};
use base64::Engine;
use std::{net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::{compression::CompressionLayer, services::ServeDir};
#[cfg(feature = "dev")]
use tower_livereload::LiveReloadLayer;
use tracing::{debug, info};

mod api;
mod components;
mod middleware;
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

    let cookie_key = base64::prelude::BASE64_STANDARD
        .decode(&opts.http_secret)
        .context("Failed to decode base64")
        .and_then(|v| tower_cookies::Key::try_from(v.as_slice()).context("Failed to import secret"))
        .context("Failed to parse HTTP_SECRET")?;

    axum::serve(
        listener,
        Router::new()
            .merge(pages_router(&state))
            .nest("/api", api_router(&state))
            .fallback_service(
                ServeDir::new(opts.http_site_root.clone())
                    .fallback(page_not_found.with_state(state.clone())),
            )
            .layer({
                let middleware = ServiceBuilder::new();
                #[cfg(feature = "dev")]
                let middleware = middleware.layer(LiveReloadLayer::new());
                middleware
            })
            .layer(
                ServiceBuilder::new()
                    .layer(axum::middleware::from_fn(middleware_http_trace))
                    .layer(CompressionLayer::new())
                    .layer(CookieManagerLayer::new())
                    .layer(axum::middleware::from_fn_with_state(
                        state.clone(),
                        middleware_error_formatting,
                    )),
            )
            .layer(Extension(cookie_key))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(await_shutdown_signal(state))
    .await
    .context("HTTP server failed to run")
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum HttpError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Not Found")]
    NotFound,
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
}
impl From<anyhow::Error> for HttpError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value.to_string())
    }
}
impl HttpError {
    fn as_status(&self) -> StatusCode {
        match self {
            HttpError::Unauthorized => StatusCode::UNAUTHORIZED,
            HttpError::NotFound => StatusCode::NOT_FOUND,
            HttpError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HttpError::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }
    fn title(&self) -> String {
        self.as_status().to_string()
    }
    fn description(&self) -> String {
        match self {
            Self::Unauthorized => "Unauthorized to access the requested resource",
            Self::NotFound => "The requested resource was not found",
            Self::Internal(_) => {
                "An internal error occurred while attempting to facilitate your request"
            }
            Self::BadRequest(msg) => msg.as_str(),
        }
        .to_string()
    }
}
impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let mut response = self.as_status().into_response();
        response.extensions_mut().insert(self);
        response
    }
}
