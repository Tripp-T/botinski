use std::sync::Arc;

use crate::Opts;
#[cfg(feature = "dev")]
use tower_livereload::LiveReloadLayer;

use {
    crate::{
        AppState,
        http::templates::{TemplateBase, template_error},
    },
    anyhow::{Context, Result},
    axum::{
        Router, debug_handler, extract::State, handler::Handler, http::StatusCode,
        response::IntoResponse, routing::get,
    },
    maud::html,
    tower::ServiceBuilder,
    tower_cookies::CookieManagerLayer,
    tower_http::{ServiceBuilderExt, services::ServeDir},
    tracing::{debug, info},
};

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
                    .fallback(response_not_found.with_state(state.clone())),
            )
            .layer(middleware)
            .with_state(state.clone()),
    )
    .with_graceful_shutdown(await_shutdown_signal(state))
    .await
    .context("HTTP server failed to run")
}

#[debug_handler]
async fn response_not_found(_: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        tmpl.set_title("404").render(template_error(
            "page not found",
            "The requested resource could not be found",
        )),
    )
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

fn pages_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/", get(page_index))
}

#[debug_handler]
async fn page_index(_state: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    tmpl.set_title("Home").render(html! {
        p { "Hello world!!!" }
        p class="text-red-400" { "From Rust btw "}
    })
}
