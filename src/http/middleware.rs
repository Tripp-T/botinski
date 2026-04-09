use std::time::Instant;

use crate::{
    AppState,
    http::{HttpError, components::component_card, templates::TemplateBase},
};
use axum::{
    debug_middleware,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::{Instrument, debug, error, info_span};

#[debug_middleware]
pub async fn middleware_error_formatting(
    State(_): State<AppState>,
    tmpl: TemplateBase,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    if let Some(error) = response.extensions_mut().remove::<HttpError>() {
        error!("{error}");
        return (
            error.as_status(),
            tmpl.set_title(error.title()).render(component_card(
                error.title(),
                error.description(),
                true,
            )),
        )
            .into_response();
    };
    response
}

#[debug_middleware]
pub async fn middleware_http_trace(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let start_time = Instant::now();

    let span = info_span!(
        "http_request",
        path = %path,
        status_code = tracing::field::Empty,
        ms_elapsed = tracing::field::Empty,
    );

    let response = next.run(req).instrument(span.clone()).await;

    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();

    span.record("status_code", status_code);
    span.record("ms_elapsed", elapsed_ms);

    if let Some(err) = response.extensions().get::<HttpError>() {
        error!(parent: &span, "{err:?}");
    }

    debug!(parent: &span, "Request completed");

    response
}
